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
        Self {
            output_path: output_path.to_path_buf(),
            page_ratio: page_ratio.to_string(),
            image_fit: image_fit.to_string(),
        }
    }

    /// Rebuild a complete snapshot using only the supplied records and directory.
    pub fn write_slides(&self, records: &[SlideRecord], slides_dir: &Path) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        let mut slides = Vec::with_capacity(records.len());
        for record in records {
            anyhow::ensure!(
                record.slide_number > 0 && seen.insert(record.slide_number),
                "Invalid or duplicate slide number {}",
                record.slide_number
            );
            let filename = Path::new(&record.png_filename);
            anyhow::ensure!(
                filename.components().count() == 1
                    && matches!(
                        filename.components().next(),
                        Some(std::path::Component::Normal(_))
                    ),
                "PNG filename must be a single filename"
            );
            slides.push((record.slide_number, slides_dir.join(filename)));
        }
        self.write_images(&slides)
    }

    pub fn add_slide(&self, record: &SlideRecord, png_path: &Path) -> Result<()> {
        let slides_dir = png_path.parent().unwrap_or(Path::new("."));
        let mut slides = if self.output_path.exists() && record.slide_number > 1 {
            self.read_existing_slides()
                .into_iter()
                .filter(|(num, _)| *num != record.slide_number)
                .map(|(num, _)| (num, slides_dir.join(format!("slide_{num:04}.png"))))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        slides.push((record.slide_number, png_path.to_path_buf()));
        self.write_images(&slides)
    }

    fn write_images(&self, slides: &[(u32, PathBuf)]) -> Result<()> {
        // Use unique temp name to avoid conflicts with antivirus or previous crashes
        let tmp_suffix = format!(
            "tmp.{:x}.pptx",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let tmp_path = self.output_path.with_file_name(&tmp_suffix);

        let existing_slides = slides
            .iter()
            .map(|(num, _)| (*num, format!("image{num}.png")))
            .collect::<Vec<_>>();

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
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

        // Validate and embed one page at a time; never hold the entire deck's PNGs in memory.
        // A missing/corrupt image aborts this candidate, leaving the good deck intact.
        let mut images = Vec::with_capacity(slides.len());
        for (num, path) in slides {
            let bytes =
                std::fs::read(path).with_context(|| format!("Failed to read PNG {:?}", path))?;
            let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                .with_context(|| format!("Invalid PNG {:?}", path))?;
            images.push((*num, decoded.width(), decoded.height()));
            drop(decoded);
            zip_write(
                &mut zip,
                &format!("ppt/media/image{num}.png"),
                options,
                &bytes,
            )?;
        }

        for (num, img_w, img_h) in &images {
            let (slide_xml, rels_xml) = SlideXml::render(
                *num,
                &format!("image{}", num),
                *img_w,
                *img_h,
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

        let file = zip.finish()?;
        file.sync_all()?;
        drop(file);

        // std::fs::rename uses replace-existing semantics on Windows as well.
        // Never copy over the good deck: a locked destination must remain intact.
        std::fs::rename(&tmp_path, &self.output_path).with_context(|| {
            format!(
                "Failed to replace {:?}; completed deck retained at {:?}",
                self.output_path, tmp_path
            )
        })?;
        info!("PPTX updated: {} slides", slides.len());
        Ok(())
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
    use crate::model::{SlideRecord, SlideRecordInput};
    use std::collections::HashSet;
    use std::io::Read;
    use std::path::{Component, Path, PathBuf};

    #[test]
    fn snapshot_rebuilds_full_set_idempotently_from_exact_directory() {
        let (dir, writer, first) = setup_pptx_test();
        let images = dir.path().join("elsewhere");
        std::fs::create_dir(&images).unwrap();
        let mut second = first.clone();
        second.slide_number = 2;
        second.png_filename = "custom.png".into();
        std::fs::write(images.join(&first.png_filename), make_test_png()).unwrap();
        std::fs::write(images.join(&second.png_filename), make_test_png()).unwrap();
        let records = [first, second];
        writer.write_slides(&records, &images).unwrap();
        let original = std::fs::read(&writer.output_path).unwrap();
        writer.write_slides(&records, &images).unwrap();
        assert_eq!(original, std::fs::read(&writer.output_path).unwrap());
        let mut zip = ZipArchive::new(std::fs::File::open(&writer.output_path).unwrap()).unwrap();
        assert!(read_zip_text(&mut zip, "docProps/app.xml").contains("<Slides>2</Slides>"));
        assert!(zip.by_name("ppt/media/image2.png").is_ok());
        assert_internal_relationship_graph_is_closed(&writer.output_path);
    }

    #[test]
    fn snapshot_missing_or_corrupt_media_preserves_previous_deck() {
        let (dir, writer, record) = setup_pptx_test();
        let slides = dir.path().join("slides");
        writer
            .add_slide(&record, &slides.join(&record.png_filename))
            .unwrap();
        let original = std::fs::read(&writer.output_path).unwrap();
        std::fs::remove_file(slides.join(&record.png_filename)).unwrap();
        assert!(writer.write_slides(&[record.clone()], &slides).is_err());
        assert_eq!(original, std::fs::read(&writer.output_path).unwrap());
        std::fs::write(slides.join(&record.png_filename), b"not a PNG").unwrap();
        assert!(writer.write_slides(&[record], &slides).is_err());
        assert_eq!(original, std::fs::read(&writer.output_path).unwrap());
    }

    #[test]
    fn add_slide_honors_supplied_png_path() {
        let (dir, writer, record) = setup_pptx_test();
        std::fs::remove_dir_all(dir.path().join("slides")).unwrap();
        let supplied = dir.path().join("custom.png");
        let png = make_test_png();
        std::fs::write(&supplied, &png).unwrap();
        writer.add_slide(&record, &supplied).unwrap();
        let mut zip = ZipArchive::new(std::fs::File::open(&writer.output_path).unwrap()).unwrap();
        let mut embedded = Vec::new();
        zip.by_name("ppt/media/image1.png")
            .unwrap()
            .read_to_end(&mut embedded)
            .unwrap();
        assert_eq!(embedded, png);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_locked_deck_survives_failed_replacement() {
        use std::os::windows::fs::OpenOptionsExt;
        let (dir, writer, record) = setup_pptx_test();
        let slides = dir.path().join("slides");
        writer
            .write_slides(std::slice::from_ref(&record), &slides)
            .unwrap();
        let original = std::fs::read(&writer.output_path).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&writer.output_path)
            .unwrap();
        assert!(writer.write_slides(&[record], &slides).is_err());
        drop(lock);
        assert_eq!(std::fs::read(&writer.output_path).unwrap(), original);
    }

    #[test]
    fn failed_replace_keeps_completed_temporary_deck() {
        let (dir, writer, record) = setup_pptx_test();
        std::fs::create_dir(&writer.output_path).unwrap();
        std::fs::write(writer.output_path.join("untouched"), b"old data").unwrap();
        assert!(writer
            .write_slides(&[record], &dir.path().join("slides"))
            .is_err());
        assert_eq!(
            std::fs::read(writer.output_path.join("untouched")).unwrap(),
            b"old data"
        );
        let temporary = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("tmp.")
            })
            .unwrap();
        assert_internal_relationship_graph_is_closed(&temporary);
    }

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
        let record = SlideRecord::new(SlideRecordInput {
            slide_number: 1,
            png_filename: "slide_0001.png".into(),
            png_relative_path: "slides/slide_0001.png".into(),
            frame_index: 1,
            width: 2,
            height: 2,
            content_hash: "test_hash".into(),
            source_name: "Test".into(),
            monitor_name: "Monitor".into(),
        });
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
        let record1 = SlideRecord::new(SlideRecordInput {
            slide_number: 1,
            png_filename: "slide_0001.png".into(),
            png_relative_path: "slides/slide_0001.png".into(),
            frame_index: 1,
            width: 2,
            height: 2,
            content_hash: "hash1".into(),
            source_name: "Test".into(),
            monitor_name: "Monitor".into(),
        });
        writer
            .add_slide(&record1, &slides_dir.join("slide_0001.png"))
            .unwrap();
        drop(writer);

        // Slide 2 (uses read_existing_slides to re-add slide 1)
        std::fs::write(slides_dir.join("slide_0002.png"), &png_data).unwrap();
        let writer2 = PptxWriter::new(&output_path, "16:9", "fit");
        let record2 = SlideRecord::new(SlideRecordInput {
            slide_number: 2,
            png_filename: "slide_0002.png".into(),
            png_relative_path: "slides/slide_0002.png".into(),
            frame_index: 2,
            width: 2,
            height: 2,
            content_hash: "hash2".into(),
            source_name: "Test".into(),
            monitor_name: "Monitor".into(),
        });
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
            let record = SlideRecord::new(SlideRecordInput {
                slide_number: number,
                png_filename: format!("slide_{number:04}.png"),
                png_relative_path: format!("slides/slide_{number:04}.png"),
                frame_index: number as u64,
                width: 2,
                height: 2,
                content_hash: format!("hash{number}"),
                source_name: "Test".into(),
                monitor_name: "Monitor".into(),
            });
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
                let record = SlideRecord::new(SlideRecordInput {
                    slide_number: i,
                    png_filename: format!("slide_{:04}.png", i),
                    png_relative_path: format!("slides/slide_{:04}.png", i),
                    frame_index: i as u64,
                    width: 2,
                    height: 2,
                    content_hash: format!("hash{}", i),
                    source_name: "Test".into(),
                    monitor_name: "Monitor".into(),
                });
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
