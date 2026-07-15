use anyhow::{Context, Result};
use log::info;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::FileOptions;

use crate::model::SlideRecord;
use super::content_types::*;
use super::slide_xml::*;

pub struct PptxWriter {
    output_path: PathBuf,
    page_ratio: String,
    image_fit: String,
}

/// Read PNG dimensions from file (parses IHDR chunk).
fn get_png_dimensions(slides_dir: &std::path::Path, num: u32) -> Option<(u32, u32)> {
    let path = slides_dir.join(format!("slide_{:04}.png", num));
    if path.exists() {
        if let Ok(img) = std::fs::read(&path) {
            if img.len() > 24 && img[0..8] == [137, 80, 78, 71, 13, 10, 26, 10] {
                let w = u32::from_be_bytes([img[16], img[17], img[18], img[19]]);
                let h = u32::from_be_bytes([img[20], img[21], img[22], img[23]]);
                return Some((w, h));
            }
        }
    }
    None
}

/// Helper to write a file entry in the ZIP archive.
fn zip_write<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    options: FileOptions<()>,
    data: &[u8],
) -> Result<()> {
    zip.start_file(name, options)?;
    zip.write_all(data)?;
    Ok(())
}

impl PptxWriter {
    pub fn new(output_path: &Path, page_ratio: &str, image_fit: &str) -> Self {
        if output_path.exists() {
            let backup = output_path.with_extension("previous.pptx");
            let _ = std::fs::copy(output_path, &backup);
            info!("Backed up existing PPTX to {:?}", backup);
        }
        Self { output_path: output_path.to_path_buf(), page_ratio: page_ratio.to_string(), image_fit: image_fit.to_string() }
    }

    pub fn add_slide(&self, record: &SlideRecord, _png_path: &Path) -> Result<()> {
        let slide_number = record.slide_number;
        let media_name = format!("image{}.png", slide_number);
        // Use unique temp name to avoid conflicts with antivirus or previous crashes
        let tmp_suffix = format!("tmp.{:x}.pptx", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        let tmp_path = self.output_path.with_file_name(&tmp_suffix);

        let mut existing_slides: Vec<(u32, String)> = Vec::new();
        if self.output_path.exists() && record.slide_number > 1 {
            if let Ok(existing) = self.read_existing_slides() {
                existing_slides = existing;
            }
        }
        existing_slides.push((slide_number, media_name.clone()));

        let file = std::fs::File::create(&tmp_path)
            .with_context(|| format!("Failed to create tmp.pptx: {:?}", tmp_path))?;
        let mut zip = ZipWriter::new(file);

        let options: FileOptions<()> = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        zip_write(&mut zip, "[Content_Types].xml", options,
            ContentTypesXml::new(&existing_slides).to_string().as_bytes())?;
        zip_write(&mut zip, "_rels/.rels", options, RELS_DOT_RELS.as_bytes())?;
        zip_write(&mut zip, "ppt/presentation.xml", options,
            PresentationXml::new(&existing_slides).to_string().as_bytes())?;
        zip_write(&mut zip, "ppt/_rels/presentation.xml.rels", options,
            PresentationRelsXml::new(&existing_slides).to_string().as_bytes())?;
        zip_write(&mut zip, "ppt/slideMasters/slideMaster1.xml", options, SLIDE_MASTER_XML.as_bytes())?;
        zip_write(&mut zip, "ppt/slideMasters/_rels/slideMaster1.xml.rels", options, SLIDE_MASTER_RELS_XML.as_bytes())?;
        zip_write(&mut zip, "ppt/slideLayouts/slideLayout1.xml", options, SLIDE_LAYOUT_XML.as_bytes())?;
        zip_write(&mut zip, "ppt/slideLayouts/_rels/slideLayout1.xml.rels", options, SLIDE_LAYOUT_RELS_XML.as_bytes())?;
        zip_write(&mut zip, "ppt/theme/theme1.xml", options, THEME_XML.as_bytes())?;

        let slides_dir = self.output_path.parent().unwrap_or(Path::new(".")).join("slides");
        let slides_dir = if slides_dir.exists() { slides_dir } else { PathBuf::from("slides") };

        for (num, media) in &existing_slides {
            let media_path = slides_dir.join(format!("slide_{:04}.png", num));
            if media_path.exists() {
                let media_bytes = std::fs::read(&media_path)
                    .with_context(|| format!("Failed to read {:?}", media_path))?;
                zip_write(&mut zip, &format!("ppt/media/{}", media), options, &media_bytes)?;
            }
        }

        for (num, _) in &existing_slides {
            // Get image dimensions from the existing PNG file
            let img_dimensions = get_png_dimensions(&slides_dir, *num);
            let (img_w, img_h) = img_dimensions.unwrap_or((1920, 1080));
            let (slide_xml, rels_xml) = SlideXml::new(*num, &format!("image{}", num), img_w, img_h, &self.image_fit, &self.page_ratio);
            zip_write(&mut zip, &format!("ppt/slides/slide{}.xml", num), options, slide_xml.as_bytes())?;
            zip_write(&mut zip, &format!("ppt/slides/_rels/slide{}.xml.rels", num), options, rels_xml.as_bytes())?;
        }

        zip_write(&mut zip, "ppt/presProps.xml", options, PRES_PROPS_XML.as_bytes())?;
        zip_write(&mut zip, "ppt/tableStyles.xml", options, TABLE_STYLES_XML.as_bytes())?;
        zip_write(&mut zip, "ppt/viewProps.xml", options, VIEW_PROPS_XML.as_bytes())?;
        zip_write(&mut zip, "docProps/app.xml", options, DOC_PROPS_APP_XML.as_bytes())?;
        zip_write(&mut zip, "docProps/core.xml", options, DOC_PROPS_CORE_XML.as_bytes())?;

        zip.finish()?;
        
        // Atomic replace: try rename first, fall back to copy+delete
        let replace_result = std::fs::rename(&tmp_path, &self.output_path);
        match replace_result {
            Ok(()) => {},
            Err(_) => {
                // Rename failed (antivirus or cross-device), try copy+delete
                std::fs::copy(&tmp_path, &self.output_path)
                    .with_context(|| format!("Failed to copy tmp to output: {:?}", self.output_path))?;
                let _ = std::fs::remove_file(&tmp_path);
            }
        }
        info!("PPTX updated: slide {} added", slide_number);
        Ok(())
    }
    
    /// Read PNG dimensions from a saved slide file.
    fn read_png_dimensions(slides_dir: &std::path::Path, num: u32) -> Option<(u32, u32)> {
        let path = slides_dir.join(format!("slide_{:04}.png", num));
        if path.exists() {
            if let Ok(img) = std::fs::read(&path) {
                // Parse PNG header for width/height
                if img.len() > 24 && img[0..8] == [137, 80, 78, 71, 13, 10, 26, 10] {
                    let w = u32::from_be_bytes([img[16], img[17], img[18], img[19]]);
                    let h = u32::from_be_bytes([img[20], img[21], img[22], img[23]]);
                    return Some((w, h));
                }
            }
        }
        None
    }

    fn read_existing_slides(&self) -> Result<Vec<(u32, String)>> {
        let file = std::fs::File::open(&self.output_path)
            .context("Failed to open existing PPTX")?;
        let mut archive = ZipArchive::new(file)
            .context("Failed to open existing PPTX as ZIP archive")?;
        let mut slides = Vec::new();
        if let Ok(mut pres) = archive.by_name("ppt/presentation.xml") {
            let mut content = String::new();
            pres.read_to_string(&mut content)?;
            for line in content.lines() {
                if line.contains("p:sldId") {
                    if let Some(r_id) = extract_attr_value(line, "r:id=\"", "\"") {
                        if let Ok(num) = r_id.trim_start_matches("rId").parse::<u32>() {
                            if num > 0 { slides.push((num, format!("image{}.png", num))); }
                        }
                    }
                }
            }
        }
        Ok(slides)
    }
}

fn extract_attr_value(s: &str, after: &str, until: &str) -> Option<String> {
    let start = s.find(after)? + after.len();
    let end = s[start..].find(until)?;
    Some(s[start..start + end].to_string())
}
