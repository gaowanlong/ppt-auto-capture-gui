use image::DynamicImage;

pub(super) fn detect_presentation(image: &DynamicImage) -> Option<(u32, u32, u32, u32)> {
    use image::GenericImageView;
    let (original_w, original_h) = image.dimensions();
    if original_w < 100 || original_h < 100 {
        return None;
    }
    let small = image.thumbnail(240, 180).to_rgb8();
    let (w, h) = small.dimensions();
    let (w, h) = (w as usize, h as usize);
    let mut vertical = vec![vec![0u32; h + 1]; w];
    let mut horizontal = vec![vec![0u32; w + 1]; h];
    let difference = |a: &image::Rgb<u8>, b: &image::Rgb<u8>| -> u32 {
        a.0.iter()
            .zip(b.0.iter())
            .map(|(x, y)| x.abs_diff(*y) as u32)
            .sum::<u32>()
            / 3
    };
    for (x, prefix) in vertical.iter_mut().enumerate().skip(1) {
        for y in 0..h {
            prefix[y + 1] = prefix[y]
                + difference(
                    small.get_pixel(x as u32, y as u32),
                    small.get_pixel((x - 1) as u32, y as u32),
                );
        }
    }
    for (y, prefix) in horizontal.iter_mut().enumerate().skip(1) {
        for x in 0..w {
            prefix[x + 1] = prefix[x]
                + difference(
                    small.get_pixel(x as u32, y as u32),
                    small.get_pixel(x as u32, (y - 1) as u32),
                );
        }
    }
    fn candidates(prefix: &[Vec<u32>]) -> Vec<usize> {
        let mut edges: Vec<_> = (1..prefix.len()).collect();
        edges.sort_by_key(|&index| std::cmp::Reverse(*prefix[index].last().unwrap()));
        let mut selected = Vec::new();
        for edge in edges {
            if *prefix[edge].last().unwrap() == 0 {
                break;
            }
            if selected.iter().all(|&old: &usize| old.abs_diff(edge) > 2) {
                selected.push(edge);
                if selected.len() == 20 {
                    break;
                }
            }
        }
        selected.sort_unstable();
        selected
    }
    let xs = candidates(&vertical);
    let ys = candidates(&horizontal);
    let mut best: Option<(u64, usize, usize, usize, usize)> = None;
    for (i, &x0) in xs.iter().enumerate() {
        for &x1 in &xs[i + 1..] {
            let width = x1 - x0;
            for (j, &y0) in ys.iter().enumerate() {
                for &y1 in &ys[j + 1..] {
                    let height = y1 - y0;
                    if width * height * 4 < w * h {
                        continue;
                    }
                    let aspect = width as f64 / height as f64;
                    if ![16.0 / 9.0, 4.0 / 3.0]
                        .iter()
                        .any(|ratio| (aspect / ratio - 1.0).abs() < 0.05)
                    {
                        continue;
                    }
                    let strengths = [
                        (vertical[x0][y1] - vertical[x0][y0]) / height as u32,
                        (vertical[x1][y1] - vertical[x1][y0]) / height as u32,
                        (horizontal[y0][x1] - horizontal[y0][x0]) / width as u32,
                        (horizontal[y1][x1] - horizontal[y1][x0]) / width as u32,
                    ];
                    let strength = *strengths.iter().min().unwrap();
                    if strength < 30 {
                        continue;
                    }
                    let score = width as u64 * height as u64 * strength as u64;
                    if best.is_none_or(|entry| score > entry.0) {
                        best = Some((score, x0, y0, x1, y1));
                    }
                }
            }
        }
    }
    let (_, x0, y0, x1, y1) = best?;
    let x0 = (x0 as u64 * original_w as u64 / w as u64) as u32;
    let x1 = (x1 as u64 * original_w as u64 / w as u64) as u32;
    let y0 = (y0 as u64 * original_h as u64 / h as u64) as u32;
    let y1 = (y1 as u64 * original_h as u64 / h as u64) as u32;
    Some((x0, y0, x1 - x0, y1 - y0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_widescreen_slide_surrounded_by_meeting_chrome() {
        let mut screenshot = image::RgbImage::from_pixel(400, 300, image::Rgb([35, 35, 35]));
        for y in 60..240 {
            for x in 40..360 {
                screenshot.put_pixel(x, y, image::Rgb([235, 235, 235]));
            }
        }
        for y in 90..110 {
            for x in 370..390 {
                screenshot.put_pixel(x, y, image::Rgb([120, 70, 60]));
            }
        }
        let (x, y, w, h) = detect_presentation(&DynamicImage::ImageRgb8(screenshot)).unwrap();
        assert!((x as i32 - 40).abs() <= 3);
        assert!((y as i32 - 60).abs() <= 3);
        assert!((w as i32 - 320).abs() <= 5);
        assert!((h as i32 - 180).abs() <= 5);
    }

    #[test]
    fn does_not_crop_when_a_clear_presentation_boundary_is_absent() {
        let screenshot = image::RgbImage::from_pixel(400, 300, image::Rgb([230, 230, 230]));
        assert!(detect_presentation(&DynamicImage::ImageRgb8(screenshot)).is_none());
    }
}
