use anyhow::{Context, Result};
use log::info;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::FileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

use super::content_types::*;
use super::slide_xml::*;
use crate::model::SlideRecord;

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
        Self {
            output_path: output_path.to_path_buf(),
            page_ratio: page_ratio.to_string(),
            image_fit: image_fit.to_string(),
        }
    }

    pub fn add_slide(&self, record: &SlideRecord, _png_path: &Path) -> Result<()> {
        let slide_number = record.slide_number;
        let media_name = format!("image{}.png", slide_number);
        // Use unique temp name to avoid conflicts with antivirus or previous crashes
        let tmp_suffix = format!(
            "tmp.{:x}.pptx",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let tmp_path = self.output_path.with_file_name(&tmp_suffix);

        let mut existing_slides: Vec<(u32, String)> = Vec::new();
        if self.output_path.exists() && record.slide_number > 1 {
            existing_slides = self.read_existing_slides();
        }
        existing_slides.push((slide_number, media_name.clone()));

        let file = std::fs::File::create(&tmp_path)
            .with_context(|| format!("Failed to create tmp.pptx: {:?}", tmp_path))?;
        let mut zip = ZipWriter::new(file);

        let options: FileOptions<()> = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        zip_write(
            &mut zip,
            "[Content_Types].xml",
            options,
            ContentTypesXml::new(&existing_slides)
                .to_string()
                .as_bytes(),
        )?;
        zip_write(&mut zip, "_rels/.rels", options, RELS_DOT_RELS.as_bytes())?;
        zip_write(
            &mut zip,
            "ppt/presentation.xml",
            options,
            PresentationXml::render(&existing_slides, &self.page_ratio)
                .to_string()
                .as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/_rels/presentation.xml.rels",
            options,
            PresentationRelsXml::render(&existing_slides)
                .to_string()
                .as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/slideMasters/slideMaster1.xml",
            options,
            SLIDE_MASTER_XML.as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            options,
            SLIDE_MASTER_RELS_XML.as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/slideLayouts/slideLayout1.xml",
            options,
            SLIDE_LAYOUT_XML.as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            options,
            SLIDE_LAYOUT_RELS_XML.as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/theme/theme1.xml",
            options,
            THEME_XML.as_bytes(),
        )?;

        let slides_dir = self
            .output_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("slides");
        let slides_dir = if slides_dir.exists() {
            slides_dir
        } else {
            PathBuf::from("slides")
        };

        for (num, media) in &existing_slides {
            let media_path = slides_dir.join(format!("slide_{:04}.png", num));
            if media_path.exists() {
                let media_bytes = std::fs::read(&media_path)
                    .with_context(|| format!("Failed to read {:?}", media_path))?;
                zip_write(
                    &mut zip,
                    &format!("ppt/media/{}", media),
                    options,
                    &media_bytes,
                )?;
            }
        }

        for (num, _) in &existing_slides {
            // Get image dimensions from the existing PNG file
            let img_dimensions = get_png_dimensions(&slides_dir, *num);
            // If PNG not found on disk, fall back to the record's stored dimensions
            let (img_w, img_h) =
                img_dimensions.unwrap_or_else(|| (record.width.max(1), record.height.max(1)));
            let (slide_xml, rels_xml) = SlideXml::render(
                *num,
                &format!("image{}", num),
                img_w,
                img_h,
                &self.image_fit,
                &self.page_ratio,
            );
            zip_write(
                &mut zip,
                &format!("ppt/slides/slide{}.xml", num),
                options,
                slide_xml.as_bytes(),
            )?;
            zip_write(
                &mut zip,
                &format!("ppt/slides/_rels/slide{}.xml.rels", num),
                options,
                rels_xml.as_bytes(),
            )?;
        }

        zip_write(
            &mut zip,
            "ppt/presProps.xml",
            options,
            PRES_PROPS_XML.as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/tableStyles.xml",
            options,
            TABLE_STYLES_XML.as_bytes(),
        )?;
        zip_write(
            &mut zip,
            "ppt/viewProps.xml",
            options,
            VIEW_PROPS_XML.as_bytes(),
        )?;
        // app.xml with correct slide count (critical: static template reports 0 slides)
        let app_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>PPT Auto Capture</Application>
  <Slides>{}</Slides>
</Properties>"#,
            existing_slides.len()
        );
        zip_write(&mut zip, "docProps/app.xml", options, app_xml.as_bytes())?;

        zip_write(
            &mut zip,
            "docProps/core.xml",
            options,
            DOC_PROPS_CORE_XML.as_bytes(),
        )?;

        zip.finish()?;

        // Atomic replace: try rename first, fall back to copy+delete
        let replace_result = std::fs::rename(&tmp_path, &self.output_path);
        match replace_result {
            Ok(()) => {}
            Err(_) => {
                // Rename failed (antivirus or cross-device), try copy+delete
                std::fs::copy(&tmp_path, &self.output_path).with_context(|| {
                    format!("Failed to copy tmp to output: {:?}", self.output_path)
                })?;
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

    fn read_existing_slides(&self) -> Vec<(u32, String)> {
        let file = match std::fs::File::open(&self.output_path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("Cannot open existing PPTX: {}", e);
                return Vec::new();
            }
        };
        let mut archive = match ZipArchive::new(file) {
            Ok(a) => a,
            Err(e) => {
                log::warn!("Cannot open existing PPTX as ZIP (corrupt?): {}", e);
                return Vec::new();
            }
        };
        let mut slides = Vec::new();
        if let Ok(mut pres) = archive.by_name("ppt/presentation.xml") {
            let mut content = String::new();
            if pres.read_to_string(&mut content).is_ok() {
                for line in content.lines() {
                    if line.contains("p:sldId") {
                        // Use id attribute (255+slide_num) to find slide number
                        if let Some(id_val) = extract_attr_value(line, "id=\"", "\"") {
                            if let Ok(raw_id) = id_val.parse::<u32>() {
                                if raw_id > 255 {
                                    let num = raw_id - 255;
                                    slides.push((num, format!("image{}.png", num)));
                                }
                            }
                        }
                    }
                }
            }
        }
        slides
    }
}

fn extract_attr_value(s: &str, after: &str, until: &str) -> Option<String> {
    let start = s.find(after)? + after.len();
    let end = s[start..].find(until)?;
    Some(s[start..start + end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SlideRecord;
    use std::collections::HashSet;
    use std::io::Read;
    use std::path::{Component, Path, PathBuf};

    /// Create a standards-compliant 2x2 RGB PNG in memory.
    fn make_test_png() -> Vec<u8> {
        use image::ImageEncoder;

        let pixels = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&pixels, 2, 2, image::ExtendedColorType::Rgb8)
            .unwrap();
        png
    }

    /// Build a temp directory with a test PNG and return the PptxWriter.
    fn setup_pptx_test() -> (tempfile::TempDir, PptxWriter, SlideRecord) {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        // Create slides/ subdirectory
        let slides_dir = dir.path().join("slides");
        std::fs::create_dir_all(&slides_dir).unwrap();
        // Write a test PNG
        let png_path = slides_dir.join("slide_0001.png");
        let png_data = make_test_png();
        std::fs::write(&png_path, &png_data).unwrap();
        // Create output path
        let output_path = dir.path().join("output.pptx");
        let writer = PptxWriter::new(&output_path, "16:9", "fit");
        // Create slide record
        let record = SlideRecord::new(
            1,
            "slide_0001.png".into(),
            "slides/slide_0001.png".into(),
            1,
            2,
            2,
            "test_hash".into(),
            "Test".into(),
            "Monitor".into(),
        );
        (dir, writer, record)
    }

    fn read_zip_text(archive: &mut ZipArchive<std::fs::File>, name: &str) -> String {
        let mut content = String::new();
        archive
            .by_name(name)
            .unwrap_or_else(|_| panic!("missing ZIP part {name}"))
            .read_to_string(&mut content)
            .unwrap_or_else(|_| panic!("failed to read ZIP part {name}"));
        content
    }

    fn relationship_owner_directory(relationship_part: &str) -> PathBuf {
        if relationship_part == "_rels/.rels" {
            return PathBuf::new();
        }

        let (prefix, relationship_name) = relationship_part
            .rsplit_once("/_rels/")
            .unwrap_or_else(|| panic!("invalid relationship part path: {relationship_part}"));
        let owner_name = relationship_name
            .strip_suffix(".rels")
            .unwrap_or_else(|| panic!("invalid relationship part suffix: {relationship_part}"));
        Path::new(prefix)
            .join(owner_name)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf()
    }

    fn normalize_package_target(base: &Path, target: &str) -> String {
        let mut normalized = Vec::new();
        for component in base.join(target).components() {
            match component {
                Component::Normal(part) => normalized.push(part.to_string_lossy().into_owned()),
                Component::ParentDir => {
                    assert!(
                        normalized.pop().is_some(),
                        "relationship target escapes package root: {target}"
                    );
                }
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) => {
                    panic!("relationship target must be package-relative: {target}")
                }
            }
        }
        normalized.join("/")
    }

    fn assert_internal_relationship_graph_is_closed(output_path: &Path) {
        let file = std::fs::File::open(output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let names = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect::<HashSet<_>>();
        let relationship_parts = names
            .iter()
            .filter(|name| name.ends_with(".rels"))
            .cloned()
            .collect::<Vec<_>>();

        for relationship_part in relationship_parts {
            let content = read_zip_text(&mut archive, &relationship_part);
            let base = relationship_owner_directory(&relationship_part);
            let mut ids = HashSet::new();
            for relationship in content.split("<Relationship ").skip(1) {
                let relationship = relationship
                    .split("/>")
                    .next()
                    .expect("relationship element must close");
                let id = extract_attr_value(relationship, "Id=\"", "\"")
                    .unwrap_or_else(|| panic!("{relationship_part} relationship missing Id"));
                assert!(
                    ids.insert(id.clone()),
                    "{relationship_part} contains duplicate relationship ID {id}"
                );
                if relationship.contains("TargetMode=\"External\"") {
                    continue;
                }
                let target = extract_attr_value(relationship, "Target=\"", "\"")
                    .unwrap_or_else(|| panic!("{relationship_part} relationship missing Target"));
                let resolved = normalize_package_target(&base, &target);
                assert!(
                    names.contains(&resolved),
                    "{relationship_part} target {target} resolves to missing part {resolved}"
                );
            }
        }
    }

    #[test]
    fn test_pptx_has_required_parts() {
        let (_dir, writer, record) = setup_pptx_test();
        let png_path = _dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let output_path = _dir.path().join("output.pptx");
        assert!(
            output_path.exists(),
            "PPTX file should exist after adding a slide"
        );

        let file = std::fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();

        // Check all required entries exist
        let required = [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slides/slide1.xml",
            "ppt/slides/_rels/slide1.xml.rels",
            "ppt/media/image1.png",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/theme/theme1.xml",
            "ppt/presProps.xml",
            "ppt/tableStyles.xml",
            "ppt/viewProps.xml",
            "docProps/app.xml",
            "docProps/core.xml",
        ];
        for name in &required {
            assert!(
                archive.by_name(name).is_ok(),
                "Missing required part: {}",
                name
            );
        }
    }

    #[test]
    fn test_pptx_xml_well_formed() {
        let (_dir, writer, record) = setup_pptx_test();
        let png_path = _dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let output_path = _dir.path().join("output.pptx");
        let file = std::fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();

        let xml_files = (0..archive.len())
            .filter_map(|index| {
                let name = archive.by_index(index).unwrap().name().to_string();
                (name.ends_with(".xml") || name.ends_with(".rels")).then_some(name)
            })
            .collect::<Vec<_>>();

        for name in xml_files {
            let content = read_zip_text(&mut archive, &name);
            let mut reader = quick_xml::Reader::from_str(&content);
            loop {
                match reader.read_event() {
                    Ok(quick_xml::events::Event::Eof) => break,
                    Ok(_) => {}
                    Err(error) => panic!(
                        "{name} is not well-formed XML at byte {}: {error}",
                        reader.buffer_position()
                    ),
                }
            }
        }
    }

    #[test]
    fn test_pptx_relationships_consistent() {
        let (_dir, writer, record) = setup_pptx_test();
        let png_path = _dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let output_path = _dir.path().join("output.pptx");
        let file = std::fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();

        // Read presentation.xml.rels
        let mut rels_content = String::new();
        archive
            .by_name("ppt/_rels/presentation.xml.rels")
            .unwrap()
            .read_to_string(&mut rels_content)
            .unwrap();

        // Check that slide relationship exists
        assert!(
            rels_content.contains("slide1.xml"),
            "Relationships should reference slide1.xml"
        );

        // Read presentation.xml to check slide ID
        let mut pres_content = String::new();
        archive
            .by_name("ppt/presentation.xml")
            .unwrap()
            .read_to_string(&mut pres_content)
            .unwrap();
        assert!(
            pres_content.contains("sldId"),
            "Presentation should contain slide ID entries"
        );
    }

    #[test]
    fn generated_pptx_has_required_presentation_relationships() {
        let (dir, writer, record) = setup_pptx_test();
        let png_path = dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let file = std::fs::File::open(dir.path().join("output.pptx")).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let rels = read_zip_text(&mut archive, "ppt/_rels/presentation.xml.rels");
        for (relationship_type, target) in [
            ("presProps", "presProps.xml"),
            ("viewProps", "viewProps.xml"),
            ("theme", "theme/theme1.xml"),
            ("tableStyles", "tableStyles.xml"),
        ] {
            assert!(
                rels.contains(&format!(
                    "/relationships/{relationship_type}\" Target=\"{target}\""
                )),
                "generated presentation is missing {relationship_type} relationship to {target}"
            );
        }
    }

    #[test]
    fn generated_pptx_has_closed_internal_relationship_graph() {
        let (dir, writer, record) = setup_pptx_test();
        let png_path = dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        assert_internal_relationship_graph_is_closed(&dir.path().join("output.pptx"));
    }

    #[test]
    fn pptx_slide_relationships_resolve_required_parts() {
        let (dir, writer, record) = setup_pptx_test();
        let png_path = dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let file = std::fs::File::open(dir.path().join("output.pptx")).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut rels = String::new();
        archive
            .by_name("ppt/slides/_rels/slide1.xml.rels")
            .unwrap()
            .read_to_string(&mut rels)
            .unwrap();

        assert!(
            rels.contains(
                r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image""#
            ) && rels.contains(r#"Target="../media/image1.png""#),
            "slide must relate to its embedded image"
        );
        assert!(
            rels.contains(
                r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout""#
            ) && rels.contains(r#"Target="../slideLayouts/slideLayout1.xml""#),
            "slide must relate to the slide layout part"
        );
        assert!(
            archive.by_name("ppt/slideLayouts/slideLayout1.xml").is_ok(),
            "slide layout relationship target must exist"
        );
    }

    #[test]
    fn pptx_master_and_layout_have_required_structure() {
        let (dir, writer, record) = setup_pptx_test();
        let png_path = dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let file = std::fs::File::open(dir.path().join("output.pptx")).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut master = String::new();
        let mut layout = String::new();
        archive
            .by_name("ppt/slideMasters/slideMaster1.xml")
            .unwrap()
            .read_to_string(&mut master)
            .unwrap();
        archive
            .by_name("ppt/slideLayouts/slideLayout1.xml")
            .unwrap()
            .read_to_string(&mut layout)
            .unwrap();

        assert!(
            master.contains("<p:cSld") && master.contains("<p:spTree>"),
            "slide master common slide data must contain a shape tree"
        );
        assert!(
            master.contains("<p:clrMap "),
            "slide master must define its color mapping"
        );
        assert!(
            master.contains(r#"<p:sldLayoutId id="2147483649" r:id="rId1"/>"#),
            "slide master must identify the layout exposed by rId1"
        );
        assert!(
            layout.contains("<p:cSld") && layout.contains("<p:spTree>"),
            "slide layout common slide data must contain a shape tree"
        );
    }

    #[test]
    fn pptx_media_image_is_decodable() {
        let (_dir, writer, record) = setup_pptx_test();
        let png_path = _dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let output_path = _dir.path().join("output.pptx");
        let file = std::fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();

        // Read the embedded image and verify it's a valid PNG
        let mut media_data = Vec::new();
        archive
            .by_name("ppt/media/image1.png")
            .unwrap()
            .read_to_end(&mut media_data)
            .unwrap();
        let image = image::load_from_memory_with_format(&media_data, image::ImageFormat::Png)
            .expect("embedded PNG must be fully decodable");
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 2);
    }

    #[test]
    fn test_pptx_multiple_slides_preserved() {
        // Simulate sequential slide captures: add slide 1, then slide 2,
        // and verify both are preserved in the final PPTX.
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let slides_dir = dir.path().join("slides");
        std::fs::create_dir_all(&slides_dir).unwrap();
        let png_data = make_test_png();

        // Slide 1
        std::fs::write(slides_dir.join("slide_0001.png"), &png_data).unwrap();
        let output_path = dir.path().join("output.pptx");
        let writer = PptxWriter::new(&output_path, "16:9", "fit");
        let record1 = SlideRecord::new(
            1,
            "slide_0001.png".into(),
            "slides/slide_0001.png".into(),
            1,
            2,
            2,
            "hash1".into(),
            "Test".into(),
            "Monitor".into(),
        );
        writer
            .add_slide(&record1, &slides_dir.join("slide_0001.png"))
            .unwrap();
        drop(writer);

        // Slide 2 (uses read_existing_slides to re-add slide 1)
        std::fs::write(slides_dir.join("slide_0002.png"), &png_data).unwrap();
        let writer2 = PptxWriter::new(&output_path, "16:9", "fit");
        let record2 = SlideRecord::new(
            2,
            "slide_0002.png".into(),
            "slides/slide_0002.png".into(),
            2,
            2,
            2,
            "hash2".into(),
            "Test".into(),
            "Monitor".into(),
        );
        writer2
            .add_slide(&record2, &slides_dir.join("slide_0002.png"))
            .unwrap();
        drop(writer2);

        // Verify both slides in final PPTX
        let file = std::fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        assert!(
            archive.by_name("ppt/slides/slide1.xml").is_ok(),
            "Slide 1 should exist"
        );
        assert!(
            archive.by_name("ppt/slides/slide2.xml").is_ok(),
            "Slide 2 should exist"
        );

        // Verify presentation.xml has both sldId entries
        let mut pres = String::new();
        archive
            .by_name("ppt/presentation.xml")
            .unwrap()
            .read_to_string(&mut pres)
            .unwrap();
        assert!(
            pres.contains(r#"sldId id="256"#),
            "Presentation should contain sldId for slide 1"
        );
        assert!(
            pres.contains(r#"sldId id="257"#),
            "Presentation should contain sldId for slide 2"
        );
    }

    #[test]
    fn pptx_preserves_distinct_media_bytes_and_slide_mappings() {
        use image::ImageEncoder;

        let dir = tempfile::tempdir().unwrap();
        let slides_dir = dir.path().join("slides");
        std::fs::create_dir_all(&slides_dir).unwrap();
        let output_path = dir.path().join("output.pptx");
        let mut expected_media = Vec::new();

        for number in 1..=3 {
            let pixels = vec![number as u8 * 50; 2 * 2 * 3];
            let mut png = Vec::new();
            image::codecs::png::PngEncoder::new(&mut png)
                .write_image(&pixels, 2, 2, image::ExtendedColorType::Rgb8)
                .unwrap();
            std::fs::write(slides_dir.join(format!("slide_{number:04}.png")), &png).unwrap();
            expected_media.push(png);

            let writer = PptxWriter::new(&output_path, "16:9", "fit");
            let record = SlideRecord::new(
                number,
                format!("slide_{number:04}.png"),
                format!("slides/slide_{number:04}.png"),
                number as u64,
                2,
                2,
                format!("hash{number}"),
                "Test".into(),
                "Monitor".into(),
            );
            writer
                .add_slide(&record, &slides_dir.join(format!("slide_{number:04}.png")))
                .unwrap();
        }

        let file = std::fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        for number in 1..=3 {
            assert!(archive
                .by_name(&format!("ppt/slides/slide{number}.xml"))
                .is_ok());
            let rels = read_zip_text(
                &mut archive,
                &format!("ppt/slides/_rels/slide{number}.xml.rels"),
            );
            assert!(rels.contains(&format!("Target=\"../media/image{number}.png\"")));
            assert!(rels.contains("Target=\"../slideLayouts/slideLayout1.xml\""));

            let mut media = Vec::new();
            archive
                .by_name(&format!("ppt/media/image{number}.png"))
                .unwrap()
                .read_to_end(&mut media)
                .unwrap();
            assert_eq!(media, expected_media[(number - 1) as usize]);
        }
        drop(archive);

        assert_internal_relationship_graph_is_closed(&output_path);
    }

    /// Generates PPTX files to /tmp/pptx_test/ for manual inspection.
    /// Run: cargo test test_pptx_generate_to_tmp -- --nocapture
    #[test]
    fn test_pptx_generate_to_tmp() {
        use crate::model::SlideRecord;
        let base = std::path::Path::new("/tmp/pptx_test");
        let slides_dir = base.join("slides");
        let _ = std::fs::remove_dir_all(base);
        std::fs::create_dir_all(&slides_dir).unwrap();
        let png_data = make_test_png();
        for i in 1..=10 {
            std::fs::write(slides_dir.join(format!("slide_{:04}.png", i)), &png_data).unwrap();
        }

        let report = |name: &str, n: u32| {
            let output = base.join(name);
            let writer = PptxWriter::new(&output, "16:9", "fit");
            for i in 1..=n {
                let record = SlideRecord::new(
                    i,
                    format!("slide_{:04}.png", i),
                    format!("slides/slide_{:04}.png", i),
                    i as u64,
                    2,
                    2,
                    format!("hash{}", i),
                    "Test".into(),
                    "Monitor".into(),
                );
                writer
                    .add_slide(&record, &slides_dir.join(format!("slide_{:04}.png", i)))
                    .unwrap();
            }
            drop(writer);
            // Verify ZIP + slide count
            let file = std::fs::File::open(&output).unwrap();
            let mut archive = zip::ZipArchive::new(file).unwrap();
            let slide_count = (1..=n)
                .filter(|i| {
                    archive
                        .by_name(&format!("ppt/slides/slide{}.xml", i))
                        .is_ok()
                })
                .count();
            assert_eq!(slide_count, n as usize);
            println!(
                "  OK  {}  ({} slides, {} bytes)",
                name,
                n,
                std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0)
            );
        };

        println!("\nPPTX test files generated at: {}/", base.display());
        println!("---");
        report("test_1slide.pptx", 1);
        report("test_3slides.pptx", 3);
        report("test_10slides.pptx", 10);
        println!("---\nAll PPTX files verified. Open them in macOS Preview or PowerPoint to check integrity.\n");
    }
}
