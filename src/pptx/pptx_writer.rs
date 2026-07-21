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
            existing_slides = self.read_existing_slides();
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
            PresentationXml::new(&existing_slides, &self.page_ratio).to_string().as_bytes())?;
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
            // If PNG not found on disk, fall back to the record's stored dimensions
            let (img_w, img_h) = img_dimensions.unwrap_or_else(|| {
                (record.width.max(1), record.height.max(1))
            });
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
    use std::io::Read;

    /// Create a minimal 2x2 RGBA PNG in memory.
    fn make_test_png() -> Vec<u8> {
        // Minimal valid PNG: 2x2 pixels, RGBA (color type 6)
        // PNG signature
        let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
        // IHDR chunk: width=2, height=2, bit_depth=8, color_type=6(RGBA)
        let ihdr_data = [0u8, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes());
        ihdr.extend_from_slice(b"IHDR");
        ihdr.extend_from_slice(&ihdr_data);
        // CRC of IHDR chunk type + data
        let crc = crc32(&ihdr[4..]);  // "IHDR" + data
        ihdr.extend_from_slice(&crc.to_be_bytes());
        png.extend_from_slice(&ihdr);

        // IDAT chunk: 2x2 RGBA pixels (4 bytes per pixel = 16 bytes raw)
        // Filter byte (0) + 16 bytes pixel data = 17 bytes
        let raw: Vec<u8> = std::iter::once(0)  // filter byte: None
            .chain([255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255].iter().cloned())
            .collect();
        // Compress using zlib (deflate)
        use std::io::Write;
        let mut compressed = Vec::new();
        {
            let mut encoder = flate2::write::ZlibEncoder::new(&mut compressed, flate2::Compression::fast());
            encoder.write_all(&raw).unwrap();
            encoder.finish().unwrap();
        }
        let mut idat = Vec::new();
        idat.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
        idat.extend_from_slice(b"IDAT");
        idat.extend_from_slice(&compressed);
        let idat_crc = crc32(&idat[4..]);
        idat.extend_from_slice(&idat_crc.to_be_bytes());
        png.extend_from_slice(&idat);

        // IEND chunk
        let iend_crc = crc32(b"IEND");
        png.extend_from_slice(&[0, 0, 0, 0, 73, 69, 78, 68]);
        png.extend_from_slice(&iend_crc.to_be_bytes());
        png
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                if crc & 1 != 0 { crc = (crc >> 1) ^ 0xEDB88320; }
                else { crc >>= 1; }
            }
        }
        crc ^ 0xFFFFFFFF
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
            1, "slide_0001.png".into(), "slides/slide_0001.png".into(),
            1, 2, 2, "test_hash".into(), "Test".into(), "Monitor".into(),
        );
        (dir, writer, record)
    }

    #[test]
    fn test_pptx_has_required_parts() {
        let (_dir, writer, record) = setup_pptx_test();
        let png_path = _dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let output_path = _dir.path().join("output.pptx");
        assert!(output_path.exists(), "PPTX file should exist after adding a slide");

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
            assert!(archive.by_name(name).is_ok(), "Missing required part: {}", name);
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

        let xml_files = [
            "[Content_Types].xml",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slides/slide1.xml",
            "ppt/slides/_rels/slide1.xml.rels",
        ];
        for name in &xml_files {
            let mut entry = archive.by_name(name).unwrap();
            let mut content = String::new();
            entry.read_to_string(&mut content).unwrap();
            // Verify it starts with XML declaration or valid XML root
            assert!(content.starts_with("<?xml") || content.starts_with("<"),
                "{} should contain valid XML", name);
            // Verify it has a closing root tag
            assert!(content.contains("</"), "{} should have closing tags", name);
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
        archive.by_name("ppt/_rels/presentation.xml.rels").unwrap()
            .read_to_string(&mut rels_content).unwrap();

        // Check that slide relationship exists
        assert!(rels_content.contains("slide1.xml"), "Relationships should reference slide1.xml");

        // Read presentation.xml to check slide ID
        let mut pres_content = String::new();
        archive.by_name("ppt/presentation.xml").unwrap()
            .read_to_string(&mut pres_content).unwrap();
        assert!(pres_content.contains("sldId"), "Presentation should contain slide ID entries");
    }

    #[test]
    fn test_pptx_media_image_integrity() {
        let (_dir, writer, record) = setup_pptx_test();
        let png_path = _dir.path().join("slides").join("slide_0001.png");
        writer.add_slide(&record, &png_path).unwrap();

        let output_path = _dir.path().join("output.pptx");
        let file = std::fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();

        // Read the embedded image and verify it's a valid PNG
        let mut media_data = Vec::new();
        archive.by_name("ppt/media/image1.png").unwrap()
            .read_to_end(&mut media_data).unwrap();
        // PNG signature should be present
        assert_eq!(&media_data[..8], &[137, 80, 78, 71, 13, 10, 26, 10],
            "Embedded image should have valid PNG signature");
        // IHDR chunk should indicate 2x2 pixels
        assert_eq!(&media_data[16..20], &[0, 0, 0, 2], "PNG width should be 2");
        assert_eq!(&media_data[20..24], &[0, 0, 0, 2], "PNG height should be 2");
    }
}