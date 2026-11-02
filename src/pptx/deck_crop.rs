//! Trims the outer edges of the screenshots already embedded in a PPTX.
//!
//! Capture sessions keep the meeting window, taskbar, or letterbox bars around
//! the slide canvas. `import_directory` avoids that for new decks, but a deck
//! that was already produced needs a second pass over the finished file. This
//! module reads the package, detects the presentation area inside every picture
//! that fills a slide, replaces only those media parts, and rewrites the
//! picture geometry so the trimmed screenshot keeps its proportions. Every
//! other package part is copied byte for byte, the slide count and page size are
//! preserved, and the source file is never modified.
//!
//! A picture is left untouched when the detector is unsure, when it is too small
//! to be a slide screenshot, when it already carries a crop rectangle, or when
//! it is rotated or flipped (the offsets would no longer be axis-aligned).

use anyhow::{bail, Context, Result};
use image::{DynamicImage, GenericImageView, ImageFormat};
use log::{info, warn};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use zip::read::ZipArchive;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::crop::{detect_presentation_with_reference, FrameReference};

/// A picture must cover this share of the slide before its pixels are treated
/// as a screenshot. Logos, icons, and decorative strips stay untouched.
const MIN_SLIDE_COVERAGE: f64 = 0.35;
/// A picture covering this share of the slide and centred on it is treated as
/// full-bleed, so a "fill the area" rewrite may expand it to the whole page.
const FULL_PAGE_COVERAGE: f64 = 0.85;
const CENTRE_TOLERANCE: f64 = 0.03;
const MAX_REFERENCE_SAMPLES: usize = 8;
const REFERENCE_SIDE: u32 = 640;
const JPEG_QUALITY: u8 = 92;
/// Aspect mismatch that is snapped to the frame instead of letterboxed.
const ASPECT_TOLERANCE: f64 = 0.005;
/// Temporary folder inside the new export directory that holds trimmed pixels.
const STAGING_DIR: &str = ".staging";
const DEFAULT_SLIDE_SIZE: (i64, i64) = (9_144_000, 5_143_500);

/// Where the trimmed screenshot is put back on its slide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CropPlacement {
    /// Keep the screenshot at its original scale and shave the edges off, so the
    /// slide gains a margin but no pixel is resampled.
    KeepScale,
    /// Keep the screenshot centred and scale it up to fill its former area. A
    /// full-bleed screenshot is grown to the whole page instead, which removes
    /// the margin the letterbox bars used to occupy.
    #[default]
    FillArea,
}

/// Result of one rewrite.
#[derive(Clone, Debug)]
pub struct DeckCropSummary {
    pub output: PathBuf,
    pub slides: usize,
    /// Pictures whose edges were actually trimmed.
    pub cropped: usize,
}

/// Crop every screenshot in `source` and write a new deck under
/// `<output>/exports/<timestamp>-<uuid>/<filename>`.
pub fn crop_deck_images(
    source: &Path,
    output_dir: &Path,
    filename: &str,
    placement: CropPlacement,
    mut progress: impl FnMut(usize, usize),
) -> Result<DeckCropSummary> {
    ensure_pptx_filename(filename)?;
    if !source.is_file() {
        bail!("Cannot find the PPTX to crop: {}", source.display());
    }
    let file = File::open(source).with_context(|| format!("Cannot open {}", source.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("{} is not a readable PPTX package", source.display()))?;

    let presentation = read_part(&mut archive, "ppt/presentation.xml")
        .context("This file has no ppt/presentation.xml and is not a PowerPoint deck")?;
    let slide_size = slide_size(&presentation);
    let slides = slide_parts(&mut archive, &presentation)?;
    if slides.is_empty() {
        bail!("This PPTX contains no slides");
    }

    let mut edits = Vec::with_capacity(slides.len());
    for (index, part) in slides.iter().enumerate() {
        let xml = match read_part(&mut archive, part) {
            Ok(xml) => xml,
            Err(error) => {
                warn!("Skipping unreadable slide {part}: {error:#}");
                continue;
            }
        };
        let relationships = slide_relationships(&mut archive, part);
        edits.push(SlideEdit {
            index,
            part: part.clone(),
            pictures: scan_pictures(&xml),
            xml,
            relationships,
            fixes: Vec::new(),
        });
        progress(index + 1, slides.len());
    }

    // Only pictures that look like a slide screenshot are candidates.
    let mut candidates: Vec<Candidate> = Vec::new();
    for slide in &edits {
        for (position, picture) in slide.pictures.iter().enumerate() {
            let Some(embed) = picture.embed.as_deref() else {
                continue;
            };
            let Some(media) = slide.relationships.get(embed) else {
                continue;
            };
            if picture.already_cropped || picture.transformed {
                continue;
            }
            let Some(geometry) = picture.geometry else {
                continue;
            };
            if !geometry.is_usable(slide_size) {
                continue;
            }
            candidates.push(Candidate {
                media: media.clone(),
                slide: slide.index,
                position,
                geometry,
            });
        }
    }
    if candidates.is_empty() {
        bail!("No embedded slide screenshots were found in this PPTX");
    }

    let reference = build_reference(&mut archive, &candidates);
    let mut crops: BTreeMap<String, Option<MediaCrop>> = BTreeMap::new();
    for candidate in &candidates {
        crops.entry(candidate.media.clone()).or_insert_with(|| {
            detect_media_crop(&mut archive, &candidate.media, reference.as_ref())
        });
    }
    progress(slides.len(), slides.len());

    let directory = unique_export_directory(output_dir)?;
    let output = directory.join(filename);
    // The trimmed pixels are staged on disk before any slide is edited: a
    // picture whose media could not be re-encoded keeps both its original bytes
    // and its original offsets.
    let staged = stage_cropped_media(&mut archive, &directory, &crops);
    let mut cropped = 0;
    for candidate in &candidates {
        let Some(crop) = crops.get(&candidate.media).copied().flatten() else {
            continue;
        };
        if !staged.contains_key(&candidate.media) {
            continue;
        }
        let geometry = match placement {
            CropPlacement::KeepScale => candidate.geometry.trimmed(crop),
            CropPlacement::FillArea => candidate.geometry.filled(crop, slide_size),
        };
        if geometry == candidate.geometry {
            continue;
        }
        if let Some(slide) = edits
            .iter_mut()
            .find(|slide| slide.index == candidate.slide)
        {
            slide.fixes.push((candidate.position, geometry));
        }
        cropped += 1;
    }

    let result = write_package(&mut archive, &output, &edits, &staged)
        .and_then(|()| verify_package(&output, &slides));
    let _ = std::fs::remove_dir_all(directory.join(STAGING_DIR));
    if let Err(error) = result {
        // A half-written or unverifiable deck must not be left behind for the
        // user to open.
        let _ = std::fs::remove_dir_all(&directory);
        return Err(error);
    }

    info!(
        "Cropped {cropped} of {} pictures in {} slides",
        candidates.len(),
        slides.len()
    );
    Ok(DeckCropSummary {
        output,
        slides: slides.len(),
        cropped,
    })
}

fn ensure_pptx_filename(filename: &str) -> Result<()> {
    if !crate::storage::plain_filename(filename) || !filename.to_lowercase().ends_with(".pptx") {
        bail!("Invalid PPTX filename: {filename}");
    }
    Ok(())
}

fn unique_export_directory(output_dir: &Path) -> Result<PathBuf> {
    let parent = output_dir.join("exports");
    std::fs::create_dir_all(&parent)
        .with_context(|| format!("Cannot create {}", parent.display()))?;
    let directory = parent.join(format!(
        "{}-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&directory)
        .with_context(|| format!("Cannot create {}", directory.display()))?;
    Ok(directory)
}

// --- Package structure ---

fn read_part(archive: &mut ZipArchive<File>, name: &str) -> Result<String> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("Missing package part {name}"))?;
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .with_context(|| format!("Cannot read package part {name}"))?;
    Ok(text)
}

/// Slide parts in presentation order. Slides that are missing from the
/// relationship table cannot be edited and are dropped (PowerPoint would not
/// show them either).
fn slide_parts(archive: &mut ZipArchive<File>, presentation: &str) -> Result<Vec<String>> {
    let relationships = parse_relationships(
        &read_part(archive, "ppt/_rels/presentation.xml.rels").unwrap_or_default(),
    );
    let list = match element_bounds(presentation, 0, presentation.len(), "p:sldIdLst") {
        Some((start, end)) => start..end,
        None => return Ok(Vec::new()),
    };
    let mut parts = Vec::new();
    let mut cursor = list.start;
    while let Some((start, end)) = element_bounds(presentation, cursor, list.end, "p:sldId") {
        cursor = end;
        let tag = &presentation[start..end];
        if let Some(id) = attribute(tag, "r:id") {
            if let Some(target) = relationships.get(&id) {
                let part = resolve_target("ppt", target);
                if !parts.contains(&part) {
                    parts.push(part);
                }
            }
        }
    }
    Ok(parts)
}

fn slide_size(presentation: &str) -> (i64, i64) {
    let Some((start, end)) = element_bounds(presentation, 0, presentation.len(), "p:sldSz") else {
        return DEFAULT_SLIDE_SIZE;
    };
    let tag = &presentation[start..end];
    match (
        attribute(tag, "cx").and_then(|value| value.parse().ok()),
        attribute(tag, "cy").and_then(|value| value.parse().ok()),
    ) {
        (Some(cx), Some(cy)) if cx > 0 && cy > 0 => (cx, cy),
        _ => DEFAULT_SLIDE_SIZE,
    }
}

/// Relationship ID to target for one slide part.
fn slide_relationships(archive: &mut ZipArchive<File>, part: &str) -> BTreeMap<String, String> {
    let rels = relay_path(part);
    let Ok(text) = read_part(archive, &rels) else {
        return BTreeMap::new();
    };
    parse_relationships(&text)
        .into_iter()
        .filter(|(_, target)| !target.starts_with("http://") && !target.starts_with("https://"))
        .map(|(id, target)| (id, resolve_target(&parent_dir(part), &target)))
        .collect()
}

fn parse_relationships(xml: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut cursor = 0;
    while let Some((start, end)) = element_bounds(xml, cursor, xml.len(), "Relationship") {
        cursor = end;
        let tag = &xml[start..end];
        if tag.contains("TargetMode=\"External\"") {
            continue;
        }
        if let (Some(id), Some(target)) = (attribute(tag, "Id"), attribute(tag, "Target")) {
            map.insert(id, target);
        }
    }
    map
}

fn relay_path(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((directory, name)) => format!("{directory}/_rels/{name}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

fn parent_dir(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((directory, _)) => directory.to_string(),
        None => String::new(),
    }
}

/// Resolve an OPC relationship target against the directory of its owner.
fn resolve_target(base: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        base.split('/').filter(|part| !part.is_empty()).collect()
    };
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn verify_package(output: &Path, slides: &[String]) -> Result<()> {
    let file = File::open(output)
        .with_context(|| format!("Cannot reopen the generated deck {}", output.display()))?;
    let mut archive = ZipArchive::new(file).with_context(|| {
        format!(
            "The generated deck {} is not a valid package",
            output.display()
        )
    })?;
    for required in ["[Content_Types].xml", "ppt/presentation.xml"] {
        if archive.by_name(required).is_err() {
            bail!("The generated deck is missing {required}");
        }
    }
    for part in slides {
        if archive.by_name(part).is_err() {
            bail!("The generated deck is missing {part}");
        }
    }
    Ok(())
}

// --- Slide XML ---

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

impl Geometry {
    fn is_usable(self, page: (i64, i64)) -> bool {
        self.cx > 0 && self.cy > 0 && coverage(self, page) >= MIN_SLIDE_COVERAGE
    }

    /// True when the picture sits on the page like a full-bleed screenshot.
    fn covers_page(self, page: (i64, i64)) -> bool {
        let (page_w, page_h) = (page.0 as f64, page.1 as f64);
        let dx = self.x as f64 + self.cx as f64 / 2.0 - page_w / 2.0;
        let dy = self.y as f64 + self.cy as f64 / 2.0 - page_h / 2.0;
        coverage(self, page) >= FULL_PAGE_COVERAGE
            && dx.abs() <= page_w * CENTRE_TOLERANCE
            && dy.abs() <= page_h * CENTRE_TOLERANCE
    }

    /// Shave the edges off at the original scale: the visible pixels keep their
    /// size, only the frame shrinks.
    fn trimmed(self, crop: MediaCrop) -> Self {
        let scale_x = self.cx as f64 / crop.source.0 as f64;
        let scale_y = self.cy as f64 / crop.source.1 as f64;
        Self {
            x: self.x + (crop.rect.0 as f64 * scale_x).round() as i64,
            y: self.y + (crop.rect.1 as f64 * scale_y).round() as i64,
            cx: ((crop.rect.2 as f64 * scale_x).round() as i64).max(1),
            cy: ((crop.rect.3 as f64 * scale_y).round() as i64).max(1),
        }
    }

    /// Scale the trimmed screenshot to fill its former area (or the whole page
    /// for a full-bleed picture) without distorting it.
    fn filled(self, crop: MediaCrop, page: (i64, i64)) -> Self {
        let (x, y, cx, cy) = if self.covers_page(page) {
            (0, 0, page.0, page.1)
        } else {
            (self.x, self.y, self.cx, self.cy)
        };
        let scale_x = cx as f64 / crop.rect.2 as f64;
        let scale_y = cy as f64 / crop.rect.3 as f64;
        let scale = scale_x.min(scale_y);
        // A crop that already matches the frame within half a percent is
        // snapped to it, so a full-page screenshot cannot leave a hairline of
        // page background around itself.
        if (scale_x / scale_y - 1.0).abs() <= ASPECT_TOLERANCE {
            return Self { x, y, cx, cy };
        }
        let width = ((crop.rect.2 as f64 * scale).round() as i64).max(1);
        let height = ((crop.rect.3 as f64 * scale).round() as i64).max(1);
        Self {
            x: x + (cx - width) / 2,
            y: y + (cy - height) / 2,
            cx: width,
            cy: height,
        }
    }
}

fn coverage(geometry: Geometry, page: (i64, i64)) -> f64 {
    let area = geometry.cx as f64 * geometry.cy as f64;
    area / (page.0 as f64 * page.1 as f64)
}

/// One `<p:pic>` element found in a slide.
#[derive(Debug)]
struct Picture {
    embed: Option<String>,
    geometry: Option<Geometry>,
    off_span: Option<Range<usize>>,
    ext_span: Option<Range<usize>>,
    already_cropped: bool,
    transformed: bool,
}

fn scan_pictures(xml: &str) -> Vec<Picture> {
    let mut pictures = Vec::new();
    let mut cursor = 0;
    while let Some((start, end)) = element_bounds(xml, cursor, xml.len(), "p:pic") {
        cursor = end;
        let block = &xml[start..end];
        let embed = element_bounds(block, 0, block.len(), "a:blip")
            .and_then(|(blip_start, blip_end)| attribute(&block[blip_start..blip_end], "r:embed"));
        let mut geometry = None;
        let mut off_span = None;
        let mut ext_span = None;
        let mut transformed = false;
        if let Some((sp_start, sp_end)) = element_bounds(block, 0, block.len(), "p:spPr") {
            if let Some((xfrm_start, xfrm_end)) = element_bounds(block, sp_start, sp_end, "a:xfrm")
            {
                let tag = &block[xfrm_start..xfrm_end];
                transformed = ["rot", "flipH", "flipV"]
                    .iter()
                    .any(|name| attribute(tag, name).is_some());
                if let (Some((off_start, off_end)), Some((ext_start, ext_end))) = (
                    element_bounds(block, xfrm_start, xfrm_end, "a:off"),
                    element_bounds(block, xfrm_start, xfrm_end, "a:ext"),
                ) {
                    let off = &block[off_start..off_end];
                    let ext = &block[ext_start..ext_end];
                    let values = (
                        attribute(off, "x").and_then(|value| value.parse::<i64>().ok()),
                        attribute(off, "y").and_then(|value| value.parse::<i64>().ok()),
                        attribute(ext, "cx").and_then(|value| value.parse::<i64>().ok()),
                        attribute(ext, "cy").and_then(|value| value.parse::<i64>().ok()),
                    );
                    if let (Some(x), Some(y), Some(cx), Some(cy)) = values {
                        geometry = Some(Geometry { x, y, cx, cy });
                        off_span = Some(start + off_start..start + off_end);
                        ext_span = Some(start + ext_start..start + ext_end);
                    }
                }
            }
        }
        pictures.push(Picture {
            embed,
            geometry,
            off_span,
            ext_span,
            already_cropped: has_crop_rectangle(block),
            transformed,
        });
    }
    pictures
}

/// True when the picture already shows only part of its image. PowerPoint
/// writes an all-zero `a:srcRect` on some round-trips, which is not a crop.
fn has_crop_rectangle(block: &str) -> bool {
    let Some((start, end)) = element_bounds(block, 0, block.len(), "a:srcRect") else {
        return false;
    };
    let tag = &block[start..end];
    ["l", "t", "r", "b"].iter().any(|name| {
        attribute(tag, name)
            .and_then(|value| value.trim().parse::<i64>().ok())
            .is_some_and(|value| value != 0)
    })
}

struct SlideEdit {
    index: usize,
    part: String,
    xml: String,
    pictures: Vec<Picture>,
    relationships: BTreeMap<String, String>,
    fixes: Vec<(usize, Geometry)>,
}

impl SlideEdit {
    /// The slide XML with the rewritten picture offsets and extents.
    fn render(&self) -> String {
        let mut spans: Vec<(Range<usize>, String)> = Vec::new();
        for (position, geometry) in &self.fixes {
            let Some(picture) = self.pictures.get(*position) else {
                continue;
            };
            if let Some(span) = picture.off_span.clone() {
                spans.push((
                    span,
                    format!("<a:off x=\"{}\" y=\"{}\"/>", geometry.x, geometry.y),
                ));
            }
            if let Some(span) = picture.ext_span.clone() {
                spans.push((
                    span,
                    format!("<a:ext cx=\"{}\" cy=\"{}\"/>", geometry.cx, geometry.cy),
                ));
            }
        }
        if spans.is_empty() {
            return self.xml.clone();
        }
        spans.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
        let mut xml = self.xml.clone();
        for (span, replacement) in spans {
            xml.replace_range(span, &replacement);
        }
        xml
    }
}

// --- Crop analysis ---

struct Candidate {
    media: String,
    slide: usize,
    position: usize,
    geometry: Geometry,
}

#[derive(Clone, Copy)]
struct MediaCrop {
    /// Inclusive origin plus width and height, in source pixels.
    rect: (u32, u32, u32, u32),
    source: (u32, u32),
}

/// Sample a few screenshots so the detector can tell a static taskbar or
/// meeting control strip from slide content that changes between slides.
fn build_reference(
    archive: &mut ZipArchive<File>,
    candidates: &[Candidate],
) -> Option<FrameReference> {
    let media: Vec<&String> = {
        let mut unique: Vec<&String> = Vec::new();
        for candidate in candidates {
            if !unique.contains(&&candidate.media) {
                unique.push(&candidate.media);
            }
        }
        unique
    };
    if media.len() < 2 {
        return None;
    }
    let sample_count = media.len().min(MAX_REFERENCE_SAMPLES);
    let mut images = Vec::with_capacity(sample_count);
    let mut full = None;
    for slot in 0..sample_count {
        let index = slot * (media.len() - 1) / (sample_count - 1).max(1);
        let Some(decoded) = decode_media(archive, media[index]) else {
            continue;
        };
        let dimensions = decoded.dimensions();
        match full {
            Some(expected) if expected != dimensions => return None,
            Some(_) => {}
            None => full = Some(dimensions),
        }
        images.push(
            if dimensions.0 > REFERENCE_SIDE || dimensions.1 > REFERENCE_SIDE {
                decoded.thumbnail(REFERENCE_SIDE, REFERENCE_SIDE)
            } else {
                decoded
            },
        );
    }
    let (width, height) = full?;
    FrameReference::from_scaled_images(&images, width, height)
}

fn decode_media(archive: &mut ZipArchive<File>, name: &str) -> Option<DynamicImage> {
    let mut entry = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    match image::guess_format(&bytes) {
        Ok(ImageFormat::Png) | Ok(ImageFormat::Jpeg) => {}
        _ => return None,
    }
    let decoded = image::load_from_memory(&bytes).ok()?;
    let (width, height) = decoded.dimensions();
    if width < 100 || height < 100 {
        return None;
    }
    Some(decoded)
}

fn detect_media_crop(
    archive: &mut ZipArchive<File>,
    media: &str,
    reference: Option<&FrameReference>,
) -> Option<MediaCrop> {
    let decoded = decode_media(archive, media)?;
    let (width, height) = decoded.dimensions();
    let (x, y, w, h) = detect_presentation_with_reference(&decoded, reference)?;
    let trimmed_share = 1.0 - (w as f64 * h as f64) / (width as f64 * height as f64);
    if trimmed_share < 0.01 {
        return None;
    }
    Some(MediaCrop {
        rect: (x, y, w, h),
        source: (width, height),
    })
}

// --- Package rewrite ---

fn write_package(
    archive: &mut ZipArchive<File>,
    output: &Path,
    slides: &[SlideEdit],
    staged: &BTreeMap<String, PathBuf>,
) -> Result<()> {
    let temporary = output.with_extension("pptx.tmp");
    let written = write_entries(archive, &temporary, output, slides, staged);
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

fn write_entries(
    archive: &mut ZipArchive<File>,
    temporary: &Path,
    output: &Path,
    slides: &[SlideEdit],
    staged: &BTreeMap<String, PathBuf>,
) -> Result<()> {
    let file = File::create(temporary)
        .with_context(|| format!("Cannot create {}", temporary.display()))?;
    let mut writer = ZipWriter::new(file);
    let edits_by_part: BTreeMap<&str, &SlideEdit> = slides
        .iter()
        .map(|slide| (slide.part.as_str(), slide))
        .collect();

    // Content types must stay the first entry for PowerPoint.
    if let Ok(content_types) = read_bytes(archive, "[Content_Types].xml") {
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        writer.start_file("[Content_Types].xml", options)?;
        writer.write_all(&content_types)?;
    }

    for index in 0..archive.len() {
        let (name, modified) = {
            let entry = archive.by_index(index)?;
            (entry.name().to_string(), entry.last_modified())
        };
        if name == "[Content_Types].xml" || name.ends_with('/') {
            continue;
        }
        // A picture that is shared between slides keeps one media part, so the
        // part is rewritten once and every slide referring to it is updated.
        let is_media = staged.contains_key(&name);
        let compression = if is_media {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        };
        let mut options = SimpleFileOptions::default()
            .compression_method(compression)
            .unix_permissions(0o644);
        if let Some(modified) = modified {
            options = options.last_modified_time(modified);
        }
        let payload = if is_media {
            std::fs::read(&staged[&name])
                .with_context(|| format!("Cannot read the staged picture for {name}"))?
        } else if let Some(slide) = edits_by_part.get(name.as_str()) {
            slide.render().into_bytes()
        } else {
            read_bytes(archive, &name)?
        };
        writer.start_file(&name, options)?;
        writer.write_all(&payload)?;
    }

    let file = writer.finish()?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, output)
        .with_context(|| format!("Cannot publish {}", output.display()))?;
    Ok(())
}

fn read_bytes(archive: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("Missing package part {name}"))?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .with_context(|| format!("Cannot read package part {name}"))?;
    Ok(bytes)
}

/// Write every trimmed screenshot to the staging folder, one image at a time so
/// the whole deck never sits in memory. Media that cannot be re-encoded are
/// reported as failures and keep their original bytes.
fn stage_cropped_media(
    archive: &mut ZipArchive<File>,
    directory: &Path,
    crops: &BTreeMap<String, Option<MediaCrop>>,
) -> BTreeMap<String, PathBuf> {
    let mut staged = BTreeMap::new();
    let wanted: Vec<(&String, MediaCrop)> = crops
        .iter()
        .filter_map(|(media, crop)| crop.map(|crop| (media, crop)))
        .collect();
    if wanted.is_empty() {
        return staged;
    }
    let staging = directory.join(STAGING_DIR);
    if let Err(error) = std::fs::create_dir(&staging) {
        warn!("Cannot create the staging folder {staging:?}: {error}");
        return staged;
    }
    for (index, (media, crop)) in wanted.iter().enumerate() {
        let Some(encoded) = encode_cropped_media(archive, media, *crop) else {
            warn!("Keep the original picture of {media}: it could not be re-encoded");
            continue;
        };
        let path = staging.join(format!("{index}.img"));
        match std::fs::write(&path, &encoded) {
            Ok(()) => {
                staged.insert((*media).clone(), path);
            }
            Err(error) => warn!("Cannot stage the trimmed picture {media}: {error}"),
        }
    }
    staged
}

fn encode_cropped_media(
    archive: &mut ZipArchive<File>,
    name: &str,
    crop: MediaCrop,
) -> Option<Vec<u8>> {
    let bytes = read_bytes(archive, name).ok()?;
    let format = image::guess_format(&bytes).ok()?;
    let decoded = image::load_from_memory_with_format(&bytes, format).ok()?;
    let cropped = decoded.crop_imm(crop.rect.0, crop.rect.1, crop.rect.2, crop.rect.3);
    let mut encoded = Vec::new();
    match format {
        ImageFormat::Png => {
            cropped
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
                .ok()?;
        }
        ImageFormat::Jpeg => {
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY);
            encoder.encode_image(&cropped.to_rgb8()).ok()?;
        }
        _ => return None,
    }
    Some(encoded)
}

// --- Minimal XML scanning ---

/// Bounds of the `<name ...>...</name>` (or self-closing) element, from the
/// opening `<` to the end of the closing tag.
fn element_bounds(xml: &str, from: usize, to: usize, name: &str) -> Option<(usize, usize)> {
    let mut search = from;
    while let Some(offset) = xml[search..to].find(&format!("<{name}")) {
        let start = search + offset;
        let after = xml[start + name.len() + 1..].chars().next();
        if matches!(
            after,
            Some('>') | Some(' ') | Some('/') | Some('\t') | Some('\n') | Some('\r')
        ) {
            let open_end = start + xml[start..to].find('>')? + 1;
            if xml[start..open_end].ends_with("/>") {
                return Some((start, open_end));
            }
            let closing = format!("</{name}>");
            let close = start + xml[start..to].find(&closing)? + closing.len();
            return Some((start, close));
        }
        search = start + 1;
    }
    None
}

/// Value of an attribute in a single tag, accepting both quote styles.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut search = 0;
    while let Some(offset) = tag[search..].find(name) {
        let start = search + offset;
        search = start + name.len();
        let preceded_by_space = tag[..start]
            .chars()
            .next_back()
            .is_none_or(|character| character.is_whitespace() || character == '<');
        if !preceded_by_space {
            continue;
        }
        let rest = tag[search..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let value = &rest[1..];
        let end = value.find(quote)?;
        return Some(value[..end].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn slide_xml(geometry: Geometry, embed: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:pic><p:nvPicPr><p:cNvPr id="2" name="shot"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
      <p:blipFill><a:blip r:embed="{embed}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
      <p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
        <a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
    </p:pic>
  </p:spTree></p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#,
            geometry.x, geometry.y, geometry.cx, geometry.cy
        )
    }

    fn presentation_xml(slide_count: usize) -> String {
        let ids = (1..=slide_count)
            .map(|number| {
                format!(
                    r#"<p:sldId id="{}" r:id="rId{}"/>"#,
                    255 + number,
                    number + 1
                )
            })
            .collect::<String>();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldIdLst>{ids}</p:sldIdLst>
  <p:sldSz cx="9144000" cy="5143500" type="screen16x9"/>
</p:presentation>"#
        )
    }

    fn presentation_rels(slide_count: usize) -> String {
        let rels = (1..=slide_count)
            .map(|number| {
                format!(
                    r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{number}.xml"/>"#,
                    number + 1
                )
            })
            .collect::<String>();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>{rels}</Relationships>"#
        )
    }

    /// A screenshot of a dark desktop with a white slide area inside it.
    fn screenshot(frame: u8) -> Vec<u8> {
        let mut canvas = image::RgbImage::from_pixel(600, 400, image::Rgb([30, 30, 34]));
        for y in 60..340 {
            for x in 60..540 {
                canvas.put_pixel(x, y, image::Rgb([255, 255, 255]));
            }
        }
        for step in 0..3u32 {
            let color = [40 + frame * 60, (90 + step * 40) as u8, 160];
            for y in 120..200 {
                for x in 120 + step * 120..220 + step * 120 {
                    canvas.put_pixel(x, y, image::Rgb(color));
                }
            }
        }
        let mut bytes = Vec::new();
        canvas
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    /// Build a minimal PPTX whose slides each embed one screenshot.
    fn build_deck(path: &Path, geometry: Geometry, slide_count: usize) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let content_types = (1..=slide_count)
            .map(|number| {
                format!(
                    r#"<Override PartName="/ppt/slides/slide{number}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
                )
            })
            .collect::<String>();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="png" ContentType="image/png"/><Default Extension="xml" ContentType="application/xml"/>{content_types}</Types>"#
            )
            .as_bytes(),
        )
        .unwrap();
        zip.start_file("ppt/presentation.xml", options).unwrap();
        zip.write_all(presentation_xml(slide_count).as_bytes())
            .unwrap();
        zip.start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        zip.write_all(presentation_rels(slide_count).as_bytes())
            .unwrap();
        for number in 1..=slide_count {
            let part = format!("ppt/slides/slide{number}.xml");
            zip.start_file(&part, options).unwrap();
            zip.write_all(slide_xml(geometry, "rId2").as_bytes())
                .unwrap();
            let rels = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image{number}.png"/></Relationships>"#
            );
            zip.start_file(format!("ppt/slides/_rels/slide{number}.xml.rels"), options)
                .unwrap();
            zip.write_all(rels.as_bytes()).unwrap();
            zip.start_file(format!("ppt/media/image{number}.png"), options)
                .unwrap();
            zip.write_all(&screenshot(number as u8)).unwrap();
        }
        zip.finish().unwrap();
    }

    fn read_media(path: &Path, name: &str) -> Vec<u8> {
        let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut bytes = Vec::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    }

    fn read_text(path: &Path, name: &str) -> String {
        String::from_utf8(read_media(path, name)).unwrap()
    }

    fn full_page() -> Geometry {
        Geometry {
            x: 0,
            y: 0,
            cx: 9_144_000,
            cy: 5_143_500,
        }
    }

    #[test]
    fn trims_embedded_screenshots_and_keeps_the_source_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("deck.pptx");
        build_deck(&source, full_page(), 3);
        let original = std::fs::read(&source).unwrap();

        let summary = crop_deck_images(
            &source,
            dir.path(),
            "cropped.pptx",
            CropPlacement::FillArea,
            |_, _| {},
        )
        .unwrap();

        assert_eq!(summary.slides, 3);
        assert_eq!(summary.cropped, 3);
        assert_eq!(std::fs::read(&source).unwrap(), original);
        let image =
            image::load_from_memory(&read_media(&summary.output, "ppt/media/image1.png")).unwrap();
        assert!((image.width() as i32 - 480).abs() <= 6, "{}", image.width());
        assert!(
            (image.height() as i32 - 280).abs() <= 6,
            "{}",
            image.height()
        );
        // The cropped picture fills the page height and stays centred instead
        // of keeping the space the black bars used to occupy.
        let slide = read_text(&summary.output, "ppt/slides/slide1.xml");
        let geometry = scan_pictures(&slide)[0].geometry.unwrap();
        assert_eq!(geometry.cy, 5_143_500);
        let expected_width = 5_143_500.0 * 480.0 / 280.0;
        assert!(
            (geometry.cx as f64 - expected_width).abs() <= 8.0,
            "{geometry:?}"
        );
        let centre = geometry.x + geometry.cx / 2;
        assert!((centre - 4_572_000).abs() <= 8, "{geometry:?}");
        assert!(summary.output.ends_with("cropped.pptx"));
    }

    #[test]
    fn keep_scale_shrinks_the_frame_without_moving_the_content() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("deck.pptx");
        let geometry = Geometry {
            x: 0,
            y: 0,
            cx: 9_144_000,
            cy: 5_143_500,
        };
        build_deck(&source, geometry, 1);
        let summary = crop_deck_images(
            &source,
            dir.path(),
            "trimmed.pptx",
            CropPlacement::KeepScale,
            |_, _| {},
        )
        .unwrap();
        let slide = read_text(&summary.output, "ppt/slides/slide1.xml");
        // 60/600 of the width and 60/400 of the height are trimmed on each side.
        assert!(
            slide.contains(r#"<a:off x="914400" y="771525"/>"#),
            "{slide}"
        );
        assert!(
            slide.contains(r#"<a:ext cx="7315200" cy="3600450"/>"#),
            "{slide}"
        );
    }

    #[test]
    fn small_pictures_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("deck.pptx");
        // A logo-sized picture covers far less than a third of the slide.
        let geometry = Geometry {
            x: 6_000_000,
            y: 3_600_000,
            cx: 1_200_000,
            cy: 700_000,
        };
        build_deck(&source, geometry, 1);
        let summary = crop_deck_images(
            &source,
            dir.path(),
            "logo.pptx",
            CropPlacement::FillArea,
            |_, _| {},
        );
        assert!(
            summary.is_err(),
            "no screenshot-sized picture should be cropped"
        );
    }

    #[test]
    fn rotated_pictures_and_existing_crops_are_skipped() {
        let xml = slide_xml(full_page(), "rId2")
            .replace(r#"<a:xfrm><a:off"#, r#"<a:xfrm rot="1800000"><a:off"#);
        let pictures = scan_pictures(&xml);
        assert_eq!(pictures.len(), 1);
        assert!(pictures[0].transformed);

        let xml = slide_xml(full_page(), "rId2").replace("<a:blip", r#"<a:srcRect l="0"/><a:blip"#);
        let pictures = scan_pictures(&xml);
        assert!(
            !pictures[0].already_cropped,
            "an all-zero crop rectangle is not a crop"
        );

        let xml =
            slide_xml(full_page(), "rId2").replace("<a:blip", r#"<a:srcRect l="10000"/><a:blip"#);
        let pictures = scan_pictures(&xml);
        assert!(pictures[0].already_cropped);
    }

    #[test]
    fn geometry_rewrite_touches_only_the_named_picture() {
        let xml = slide_xml(full_page(), "rId2");
        let pictures = scan_pictures(&xml);
        let slide = SlideEdit {
            index: 0,
            part: "ppt/slides/slide1.xml".into(),
            xml: xml.clone(),
            pictures,
            relationships: BTreeMap::new(),
            fixes: vec![(
                0,
                Geometry {
                    x: 10,
                    y: 20,
                    cx: 30,
                    cy: 40,
                },
            )],
        };
        let rendered = slide.render();
        assert!(rendered.contains(r#"<a:off x="10" y="20"/>"#), "{rendered}");
        assert!(
            rendered.contains(r#"<a:ext cx="30" cy="40"/>"#),
            "{rendered}"
        );
        assert_eq!(rendered.matches("<a:off").count(), 1);
    }

    #[test]
    fn missing_presentation_is_reported_instead_of_producing_a_broken_deck() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("not-a-deck.pptx");
        std::fs::write(&source, b"just text").unwrap();
        let error = crop_deck_images(
            &source,
            dir.path(),
            "out.pptx",
            CropPlacement::FillArea,
            |_, _| {},
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("PPTX"), "{error:#}");
    }

    #[test]
    fn empty_slide_list_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("empty.pptx");
        let file = File::create(&source).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(b"<Types/>").unwrap();
        zip.start_file("ppt/presentation.xml", options).unwrap();
        zip.write_all(presentation_xml(0).as_bytes()).unwrap();
        zip.finish().unwrap();
        let error = crop_deck_images(
            &source,
            dir.path(),
            "out.pptx",
            CropPlacement::FillArea,
            |_, _| {},
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("no slides"), "{error:#}");
    }

    #[test]
    fn relationship_targets_resolve_inside_the_package() {
        assert_eq!(
            resolve_target("ppt/slides", "../media/image1.png"),
            "ppt/media/image1.png"
        );
        assert_eq!(
            resolve_target("ppt", "slides/slide1.xml"),
            "ppt/slides/slide1.xml"
        );
        assert_eq!(
            resolve_target("ppt", "/ppt/slides/slide1.xml"),
            "ppt/slides/slide1.xml"
        );
    }

    #[test]
    fn attributes_are_read_from_tags_with_both_quote_styles() {
        let tag = r#"<a:off x="12" y='34'/>"#;
        assert_eq!(attribute(tag, "x").as_deref(), Some("12"));
        assert_eq!(attribute(tag, "y").as_deref(), Some("34"));
        assert!(attribute(tag, "cx").is_none());
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use std::io::Read;

    /// A screenshot of a 16:10 desktop showing a 16:9 slide, with the meeting
    /// chrome and a static control strip around the canvas.
    fn write_window_frame(path: &Path, frame: u32) {
        let mut image = image::RgbImage::from_pixel(1512, 982, image::Rgb([28, 28, 30]));
        let fill = |image: &mut image::RgbImage, x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 3]| {
            for y in y0..y1 {
                for x in x0..x1 {
                    image.put_pixel(x, y, image::Rgb(c));
                }
            }
        };
        fill(&mut image, 0, 0, 1512, 64, [42, 42, 46]);
        fill(&mut image, 120, 64, 1392, 960, [0, 0, 0]);
        fill(&mut image, 120, 154, 1392, 869, [255, 255, 255]);
        fill(&mut image, 180, 220, 900, 250, [40, 60, 90]);
        for block in 0..4u32 {
            let x = 260 + ((block + frame) % 4) * 240;
            fill(&mut image, x, 420, x + 180, 620, [0, 140, 210]);
        }
        fill(&mut image, 200, 869, 1310, 915, [58, 58, 64]);
        image.save(path).unwrap();
    }

    #[test]
    fn a_deck_built_from_screenshots_is_trimmed_back_to_the_slide() {
        let base = tempfile::tempdir().unwrap();
        let shots = base.path().join("shots");
        std::fs::create_dir_all(&shots).unwrap();
        for frame in 0..3 {
            write_window_frame(&shots.join(format!("slide_{:04}.png", frame + 1)), frame);
        }
        let decks = crate::storage::import_directory(
            &shots,
            base.path(),
            "capture.pptx",
            "16:9",
            "fit",
            false,
            |_, _| {},
        )
        .unwrap();
        let untouched = std::fs::read(&decks[0]).unwrap();

        let summary = crop_deck_images(
            &decks[0],
            base.path(),
            "capture-cropped.pptx",
            CropPlacement::FillArea,
            |_, _| {},
        )
        .unwrap();

        assert_eq!(summary.slides, 3);
        assert_eq!(summary.cropped, 3);
        assert_eq!(std::fs::read(&decks[0]).unwrap(), untouched);
        // The export folder holds the finished deck only.
        let leftovers: Vec<_> = std::fs::read_dir(summary.output.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(leftovers, vec!["capture-cropped.pptx".to_string()]);
        let mut archive = ZipArchive::new(File::open(&summary.output).unwrap()).unwrap();
        for number in 1..=3 {
            let mut bytes = Vec::new();
            archive
                .by_name(&format!("ppt/media/image{number}.png"))
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            let image = image::load_from_memory(&bytes).unwrap();
            // The window chrome and the black bars are gone.
            assert!(
                (image.width() as i32 - 1272).abs() <= 6,
                "slide {number}: {}x{}",
                image.width(),
                image.height()
            );
            assert!(
                (image.height() as i32 - 715).abs() <= 6,
                "slide {number}: {}x{}",
                image.width(),
                image.height()
            );
        }
        drop(archive);

        // The trimmed screenshot now covers the page instead of leaving the
        // letterbox space around it.
        let mut archive = ZipArchive::new(File::open(&summary.output).unwrap()).unwrap();
        let mut slide = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .unwrap()
            .read_to_string(&mut slide)
            .unwrap();
        let geometry = scan_pictures(&slide)[0].geometry.unwrap();
        assert_eq!(geometry.cx, 9_144_000);
        assert_eq!(geometry.x, 0);
        assert!(geometry.cy >= 5_100_000, "{geometry:?}");
        assert!(
            (geometry.y + geometry.cy - 5_143_500).abs() <= 2,
            "{geometry:?}"
        );
    }

    #[test]
    fn a_deck_of_plain_white_pictures_is_copied_without_touching_the_media() {
        let base = tempfile::tempdir().unwrap();
        let shots = base.path().join("shots");
        std::fs::create_dir_all(&shots).unwrap();
        for number in 1..=2 {
            image::RgbImage::from_pixel(800, 450, image::Rgb([255, 255, 255]))
                .save(shots.join(format!("{number}.png")))
                .unwrap();
        }
        let decks = crate::storage::import_directory(
            &shots,
            base.path(),
            "blank.pptx",
            "16:9",
            "fit",
            false,
            |_, _| {},
        )
        .unwrap();
        let summary = crop_deck_images(
            &decks[0],
            base.path(),
            "blank-cropped.pptx",
            CropPlacement::FillArea,
            |_, _| {},
        )
        .unwrap();
        assert_eq!(summary.cropped, 0);
        let mut archive = ZipArchive::new(File::open(&summary.output).unwrap()).unwrap();
        let mut bytes = Vec::new();
        archive
            .by_name("ppt/media/image1.png")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let image = image::load_from_memory(&bytes).unwrap();
        assert_eq!((image.width(), image.height()), (800, 450));
    }
}
