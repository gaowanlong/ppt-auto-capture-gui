use super::{
    session::{part_filename, plain_filename, SLIDES_PER_DECK},
    ImageStore,
};
use crate::model::{Frame, SlideRecord, SlideRecordInput};
use crate::pptx::PptxWriter;
use anyhow::{ensure, Context, Result};
use std::path::{Path, PathBuf};

pub fn import_directory(
    source: &Path,
    output: &Path,
    filename: &str,
    ratio: &str,
    fit: &str,
    auto_crop: bool,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<PathBuf>> {
    let mut images = Vec::new();
    for entry in std::fs::read_dir(source)
        .with_context(|| format!("Cannot read image directory: {}", source.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
        if ["png", "jpg", "jpeg"]
            .iter()
            .any(|ext| extension.eq_ignore_ascii_case(ext))
        {
            images.push(path);
        }
    }
    images.sort_by(|a, b| {
        natural_compare(
            &a.file_name().unwrap().to_string_lossy(),
            &b.file_name().unwrap().to_string_lossy(),
        )
    });
    ensure!(
        !images.is_empty(),
        "No PNG or JPEG images found in {}",
        source.display()
    );
    ensure!(images.len() <= u32::MAX as usize, "Too many images");
    ensure!(
        plain_filename(filename) && filename.to_lowercase().ends_with(".pptx"),
        "Invalid PPTX filename"
    );
    let parent = output.join("exports");
    std::fs::create_dir_all(&parent)?;
    let directory = parent.join(format!(
        "{}-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&directory)?;
    let image_store = ImageStore::new(directory.clone())?;
    let mut records = Vec::with_capacity(images.len());
    let total = images.len();
    for (index, path) in images.iter().enumerate() {
        let rgba = image::open(path)
            .with_context(|| format!("Cannot read image: {}", path.display()))?
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        let mut bgra = rgba.into_raw();
        for pixel in bgra.chunks_exact_mut(4) {
            let alpha = pixel[3] as u32;
            for channel in &mut pixel[..3] {
                *channel = ((*channel as u32 * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let number = (index + 1) as u32;
        let frame = Frame::new(
            bgra,
            width,
            height,
            width.checked_mul(4).context("Image too wide")?,
            number as u64,
            0,
        );
        let record = SlideRecord::new(SlideRecordInput {
            slide_number: number,
            png_filename: format!("slide_{number:04}.png"),
            png_relative_path: format!("slides/slide_{number:04}.png"),
            frame_index: number as u64,
            width,
            height,
            content_hash: format!("import-{number}"),
            source_name: path.file_name().unwrap().to_string_lossy().into_owned(),
            monitor_name: String::new(),
        });
        image_store
            .save_png(&frame, number)
            .with_context(|| format!("Cannot import {}", path.display()))?;
        records.push(record);
        progress(index + 1, total);
    }
    let mut paths = Vec::new();
    for (part, chunk) in records.chunks(SLIDES_PER_DECK).enumerate() {
        let records: Vec<_> = chunk
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let mut entry = entry.clone();
                entry.slide_number = (index + 1) as u32;
                entry
            })
            .collect();
        let path = directory.join(part_filename(filename, part));
        PptxWriter::new(&path, ratio, fit)
            .with_auto_crop(auto_crop)
            .write_slides(&records, &directory.join("slides"))
            .with_context(|| format!("Cannot generate PPTX in {}", directory.display()))?;
        paths.push(path);
    }
    Ok(paths)
}

fn natural_compare(left: &str, right: &str) -> std::cmp::Ordering {
    let (a, b) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (start_a, start_b) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let da = &a[start_a..i];
            let db = &b[start_b..j];
            let na = da
                .iter()
                .position(|v| *v != b'0')
                .map_or(&da[da.len()..], |n| &da[n..]);
            let nb = db
                .iter()
                .position(|v| *v != b'0')
                .map_or(&db[db.len()..], |n| &db[n..]);
            let cmp = na
                .len()
                .cmp(&nb.len())
                .then_with(|| na.cmp(nb))
                .then_with(|| da.len().cmp(&db.len()));
            if !cmp.is_eq() {
                return cmp;
            }
        } else {
            let cmp = a[i].to_ascii_lowercase().cmp(&b[j].to_ascii_lowercase());
            if !cmp.is_eq() {
                return cmp;
            }
            i += 1;
            j += 1;
        }
    }
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn imports_images_in_natural_order_without_modifying_them_and_splits_at_50() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        for number in 1..=51u8 {
            image::RgbImage::from_pixel(1, 1, image::Rgb([number, 0, 0]))
                .save(source.path().join(format!("screen-{number}.png")))
                .unwrap();
        }
        std::fs::write(source.path().join("notes.txt"), b"untouched").unwrap();
        let original = std::fs::read(source.path().join("screen-1.png")).unwrap();
        let mut updates = Vec::new();
        let decks = import_directory(
            source.path(),
            output.path(),
            "slides.pptx",
            "16:9",
            "fit",
            false,
            |done, total| updates.push((done, total)),
        )
        .unwrap();
        assert_eq!(decks.len(), 2);
        assert!(decks[0].ends_with("slides.pptx"));
        assert!(decks[1].ends_with("slides-part-002.pptx"));
        assert_eq!(updates.last(), Some(&(51, 51)));
        for (path, page, expected) in [(&decks[0], 1, 1), (&decks[0], 50, 50), (&decks[1], 1, 51)] {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
            let mut bytes = Vec::new();
            zip.by_name(&format!("ppt/media/image{page}.png"))
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(
                image::load_from_memory(&bytes)
                    .unwrap()
                    .to_rgb8()
                    .get_pixel(0, 0)
                    .0,
                [expected, 0, 0]
            );
        }
        assert_eq!(
            std::fs::read(source.path().join("screen-1.png")).unwrap(),
            original
        );
        assert_eq!(
            std::fs::read(source.path().join("notes.txt")).unwrap(),
            b"untouched"
        );
    }

    #[test]
    fn rejects_an_empty_image_directory_without_creating_a_session() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        assert!(import_directory(
            source.path(),
            output.path(),
            "slides.pptx",
            "16:9",
            "fit",
            false,
            |_, _| {}
        )
        .is_err());
        assert!(!output.path().join("sessions").exists());
    }

    #[test]
    fn broken_input_does_not_create_a_recoverable_partial_recording() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        image::RgbImage::from_pixel(1, 1, image::Rgb([10, 20, 30]))
            .save(source.path().join("1.png"))
            .unwrap();
        std::fs::write(source.path().join("2.png"), b"not an image").unwrap();
        assert!(import_directory(
            source.path(),
            output.path(),
            "slides.pptx",
            "16:9",
            "fit",
            false,
            |_, _| {}
        )
        .is_err());
        assert!(crate::storage::detect_incomplete_session(output.path())
            .unwrap()
            .is_none());
        assert_eq!(
            std::fs::read(source.path().join("2.png")).unwrap(),
            b"not an image"
        );
    }

    #[test]
    fn transparent_png_is_composited_on_white_for_powerpoint() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 0]))
            .save(source.path().join("transparent.png"))
            .unwrap();
        let decks = import_directory(
            source.path(),
            output.path(),
            "slides.pptx",
            "16:9",
            "fit",
            false,
            |_, _| {},
        )
        .unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&decks[0]).unwrap()).unwrap();
        let mut bytes = Vec::new();
        zip.by_name("ppt/media/image1.png")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(
            image::load_from_memory(&bytes)
                .unwrap()
                .to_rgb8()
                .get_pixel(0, 0)
                .0,
            [255, 255, 255]
        );
    }
}
