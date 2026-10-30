//! Detects the presentation (slide) area inside a desktop screenshot.
//!
//! Meeting-app screenshots contain neutral window chrome, toolbars, participant
//! panels, taskbars, and letterbox bars around a mostly white slide canvas. The
//! detector walks inward from the image edges:
//!
//! 1. A strict adaptive white threshold is derived from the image's bright
//!    percentile; per-column and per-row white fractions are merged into a
//!    coarse canvas rectangle. White runs separated by a content gap are joined
//!    only when the slide's boundary lines continue across that gap (a photo or
//!    video spanning the slide), never when a panel or taskbar interrupts.
//! 2. The coarse rectangle is mapped back to full resolution, grown across
//!    colorful slide bands (title bars, footers, full-bleed photos) that end at
//!    neutral chrome, and its edges are snapped to the outermost positive
//!    brightness step. Snapping to a brightness step ignores embedded photo
//!    edges, whose outside is brighter than the slide canvas.
//! 3. A conservative edge fallback handles dark, full-bleed decks. It only
//!    returns a rectangle without a chrome-like border, so an ambiguous dark
//!    slide keeps the full screenshot instead of gaining black bars or losing
//!    slide content.
//!
//! Whenever a stage is uncertain the function returns `None` and the caller
//! embeds the original screenshot unchanged.

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

/// Pixel rectangle: inclusive start, exclusive end, in full-image coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Rect {
    fn width(self) -> u32 {
        self.x1 - self.x0
    }

    fn height(self) -> u32 {
        self.y1 - self.y0
    }
}

/// Luma and saturation planes for a (possibly downscaled) copy of an image.
struct Planes {
    width: usize,
    height: usize,
    luma: Vec<u8>,
    saturation: Vec<u8>,
}

impl Planes {
    fn new(image: &DynamicImage, max_side: u32) -> Self {
        let (source_width, source_height) = image.dimensions();
        let scale = (max_side as f64 / source_width.max(source_height) as f64).min(1.0);
        let width = ((source_width as f64 * scale).round() as u32).max(1);
        let height = ((source_height as f64 * scale).round() as u32).max(1);
        let resized;
        let source: &DynamicImage = if width == source_width && height == source_height {
            image
        } else {
            resized = image.resize_exact(width, height, FilterType::Lanczos3);
            &resized
        };
        let mut luma = Vec::with_capacity((width * height) as usize);
        let mut saturation = Vec::with_capacity((width * height) as usize);
        for pixel in source.pixels() {
            let [red, green, blue, _] = pixel.2 .0;
            // ITU-R BT.601 luma, matching the threshold calibration.
            luma.push(
                ((299 * red as u32 + 587 * green as u32 + 114 * blue as u32 + 500) / 1000) as u8,
            );
            let high = red.max(green).max(blue);
            let low = red.min(green).min(blue);
            saturation.push(if high == 0 {
                0
            } else {
                ((high - low) as u32 * 255 / high as u32) as u8
            });
        }
        Self {
            width: width as usize,
            height: height as usize,
            luma,
            saturation,
        }
    }

    #[inline]
    fn luma_at(&self, x: usize, y: usize) -> u8 {
        self.luma[y * self.width + x]
    }

    #[inline]
    fn saturation_at(&self, x: usize, y: usize) -> u8 {
        self.saturation[y * self.width + x]
    }

    /// Value at the given percentile of the luma histogram (0.98 selects the
    /// brightest two percent cutoff).
    fn luma_percentile(&self, fraction: f64) -> u8 {
        let mut histogram = [0u32; 256];
        for &value in &self.luma {
            histogram[value as usize] += 1;
        }
        let total = self.luma.len() as u32;
        let needed = ((total as f64 * (1.0 - fraction)) as u32).max(1);
        let mut seen = 0;
        for value in (0..=255).rev() {
            seen += histogram[value];
            if seen >= needed {
                return value as u8;
            }
        }
        0
    }
}

fn percentile_u8(values: &mut [u8], fraction: f64) -> u8 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let index = ((values.len() as f64 * fraction) as usize).min(values.len() - 1);
    values[index]
}

/// Contiguous ranges of `true` at least `min_run` long.
fn runs_of(flags: &[bool], min_run: usize) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = None;
    for (index, &flag) in flags.iter().enumerate() {
        match (flag, start) {
            (true, None) => start = Some(index),
            (false, Some(begin)) => {
                if index - begin >= min_run {
                    runs.push((begin, index));
                }
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        if flags.len() - begin >= min_run {
            runs.push((begin, flags.len()));
        }
    }
    runs
}

/// Merge white runs separated by a small gap, or by a content gap whose slide
/// boundary lines continue on both sides. Starts from the run with the most
/// white pixels so stray white UI never wins over the canvas.
fn bridge_runs(
    runs: Vec<(usize, usize)>,
    profile: &[f64],
    gap_small: usize,
    continuation: Option<&dyn Fn(usize, usize) -> f64>,
    continuation_threshold: f64,
    min_run: usize,
) -> Option<(usize, usize)> {
    let runs: Vec<(usize, usize)> = runs
        .into_iter()
        .filter(|&(start, end)| end - start >= min_run)
        .collect();
    if runs.is_empty() {
        return None;
    }
    let mut seed = 0;
    let mut seed_weight = f64::MIN;
    for (index, &(start, end)) in runs.iter().enumerate() {
        let weight: f64 = profile[start..end].iter().sum();
        if weight > seed_weight {
            seed_weight = weight;
            seed = index;
        }
    }
    let (mut start, mut end) = runs[seed];
    let mut left = seed;
    while left > 0 {
        let (previous_start, previous_end) = runs[left - 1];
        let joins = start - previous_end <= gap_small
            || continuation
                .is_some_and(|probe| probe(previous_end, start) >= continuation_threshold);
        if !joins {
            break;
        }
        start = previous_start;
        left -= 1;
    }
    let mut right = seed + 1;
    while right < runs.len() {
        let (next_start, next_end) = runs[right];
        let joins = next_start - end <= gap_small
            || continuation.is_some_and(|probe| probe(end, next_start) >= continuation_threshold);
        if !joins {
            break;
        }
        end = next_end;
        right += 1;
    }
    Some((start, end))
}

/// Mean luma, mean saturation, and luma deviation of a line of pixels.
fn line_stats(
    planes: &Planes,
    column: bool,
    position: usize,
    start: usize,
    end: usize,
) -> (f64, f64, f64) {
    let (position_limit, span_limit) = if column {
        (planes.width, planes.height)
    } else {
        (planes.height, planes.width)
    };
    if start >= end || position >= position_limit || end > span_limit {
        return (0.0, 0.0, 0.0);
    }
    let step = ((end - start) / 160).max(1);
    let mut values = Vec::with_capacity((end - start) / step + 1);
    let mut saturation_sum = 0.0;
    let mut along = start;
    while along < end {
        let (x, y) = if column {
            (position, along)
        } else {
            (along, position)
        };
        values.push(planes.luma_at(x, y) as f64);
        saturation_sum += planes.saturation_at(x, y) as f64;
        along += step;
    }
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let deviation = (values
        .iter()
        .map(|&value| (value - mean) * (value - mean))
        .sum::<f64>()
        / count)
        .sqrt();
    (mean, saturation_sum / count, deviation)
}

/// Content is either saturated (colored bands, photos, video) or high-contrast
/// non-black graphics.
fn is_content(luma: f64, saturation: f64, deviation: f64) -> bool {
    saturation >= 30.0 || (deviation >= 20.0 && luma >= 30.0)
}

/// Neutral, low-contrast pixels: window chrome, letterbox bars, panels.
fn is_chrome(saturation: f64, deviation: f64) -> bool {
    saturation < 26.0 && deviation < 18.0
}

/// Grow one rectangle edge outward across contiguous slide content, stopping at
/// neutral chrome. A short white pad is allowed because the downscaled estimate
/// can sit a few pixels inside the real edge; it is only committed when a
/// content band follows.
fn grow_edge(
    planes: &Planes,
    edge: u32,
    start: bool,
    column: bool,
    perpendicular: (usize, usize),
    limit: usize,
) -> u32 {
    let direction: i64 = if start { -1 } else { 1 };
    let mut position = edge as i64;
    let mut padding = 0;
    while padding < 6 {
        let line = position + direction;
        if !(0..limit as i64).contains(&line) {
            break;
        }
        let (luma, saturation, _) = line_stats(
            planes,
            column,
            line as usize,
            perpendicular.0,
            perpendicular.1,
        );
        if luma >= 235.0 && saturation <= 40.0 {
            position = line;
            padding += 1;
        } else {
            break;
        }
    }
    let cap = (limit as f64 * 0.6) as usize;
    let mut moved = 0;
    while moved < cap {
        let line = position + direction;
        if !(0..limit as i64).contains(&line) {
            break;
        }
        let (luma, saturation, deviation) = line_stats(
            planes,
            column,
            line as usize,
            perpendicular.0,
            perpendicular.1,
        );
        if is_content(luma, saturation, deviation) {
            position = line;
            moved += 1;
        } else {
            break;
        }
    }
    if moved == 0 || moved >= cap {
        return edge;
    }
    let far = position + direction;
    if !(0..limit as i64).contains(&far) {
        return edge;
    }
    let (_, saturation, deviation) = line_stats(
        planes,
        column,
        far as usize,
        perpendicular.0,
        perpendicular.1,
    );
    if is_chrome(saturation, deviation) {
        far as u32
    } else {
        edge
    }
}

/// Grow a rectangle across slide content bands that touch its edges.
fn expand_graphic_bands(planes: &Planes, rect: Rect) -> Rect {
    let mut result = rect;
    let horizontal_span = (rect.x0 as usize, rect.x1 as usize);
    let vertical_span = (rect.y0 as usize, rect.y1 as usize);
    result.y0 = grow_edge(planes, rect.y0, true, false, horizontal_span, planes.height);
    result.y1 = grow_edge(
        planes,
        rect.y1,
        false,
        false,
        horizontal_span,
        planes.height,
    );
    result.x0 = grow_edge(planes, rect.x0, true, true, vertical_span, planes.width);
    result.x1 = grow_edge(planes, rect.x1, false, true, vertical_span, planes.width);
    result
}

/// Snap rough edges to the outermost positive brightness step. For a white
/// canvas this picks the slide boundary and ignores darker photo edges inside
/// the slide. A strong unsigned transition is used when no brightness step
/// exists (for example a colored band meeting a same-luma chrome).
fn refine_edges(planes: &Planes, rect: Rect, scale: f64, prefer_positive: bool) -> Rect {
    let distance = ((scale * 3.0).round() as i64).max(3);
    let scan = |column: bool,
                edge: u32,
                start: bool,
                perpendicular: (usize, usize),
                limit: usize|
     -> u32 {
        let span = perpendicular.1.saturating_sub(perpendicular.0);
        if span < 5 {
            return edge;
        }
        let step = (span / 48).max(1);
        let mut positions = Vec::new();
        let mut generic = Vec::new();
        let mut across = perpendicular.0 + span / 20;
        let stop = perpendicular.1 - span / 20;
        while across < stop {
            let low = (edge as i64 - distance).max(1);
            let high = (edge as i64 + distance).min(limit as i64 - 1);
            let mut first_position = None;
            let mut first_step = 0.0;
            let mut best_position = None;
            let mut best_score = 0.0;
            let mut candidate = low;
            while candidate < high {
                let position = candidate as usize;
                if position >= 2 {
                    let (inside_a, inside_b, outside_a, outside_b) = if start {
                        (position, position + 1, position - 2, position - 1)
                    } else {
                        (position - 2, position - 1, position, position + 1)
                    };
                    let pair = |first: usize, second: usize| -> (f64, f64) {
                        let (luma_first, saturation_first, luma_second, saturation_second) =
                            if column {
                                (
                                    planes.luma_at(first, across),
                                    planes.saturation_at(first, across),
                                    planes.luma_at(second, across),
                                    planes.saturation_at(second, across),
                                )
                            } else {
                                (
                                    planes.luma_at(across, first),
                                    planes.saturation_at(across, first),
                                    planes.luma_at(across, second),
                                    planes.saturation_at(across, second),
                                )
                            };
                        (
                            (luma_first as f64 + luma_second as f64) / 2.0,
                            (saturation_first as f64 + saturation_second as f64) / 2.0,
                        )
                    };
                    let (inside_luma, inside_saturation) = pair(inside_a, inside_b);
                    let (outside_luma, outside_saturation) = pair(outside_a, outside_b);
                    let brightness_step = inside_luma - outside_luma;
                    let score = brightness_step.abs()
                        + 0.25 * (inside_saturation - outside_saturation).abs();
                    if brightness_step >= 5.0 && brightness_step > first_step {
                        first_position = Some(position);
                        first_step = brightness_step;
                    }
                    // A white canvas must not snap inward to an embedded dark
                    // photo edge; only brighter-inside transitions, small
                    // mapping corrections, and brightness-neutral (saturated)
                    // edges are trusted.
                    let near_edge = (position as i64 - edge as i64).abs() <= 2;
                    if score > best_score
                        && (!prefer_positive || brightness_step >= 0.0 || near_edge)
                    {
                        best_position = Some(position);
                        best_score = score;
                    }
                }
                candidate += 1;
            }
            if let Some(position) = first_position {
                positions.push(position);
            } else if let Some(position) = best_position {
                if best_score >= 10.0 {
                    generic.push(position);
                }
            }
            across += step;
        }
        let picks = if positions.len() >= 4 {
            positions
        } else {
            generic
        };
        if picks.len() < 4 {
            return edge;
        }
        let mut picks = picks;
        picks.sort_unstable();
        picks[picks.len() / 2] as u32
    };
    let horizontal_span = (rect.y0 as usize, rect.y1 as usize);
    let vertical_span = (rect.x0 as usize, rect.x1 as usize);
    Rect {
        x0: scan(true, rect.x0, true, horizontal_span, planes.width),
        y0: scan(false, rect.y0, true, vertical_span, planes.height),
        x1: scan(true, rect.x1, false, horizontal_span, planes.width),
        y1: scan(false, rect.y1, false, vertical_span, planes.height),
    }
}

/// A fallback rectangle that still contains letterbox or control bands is
/// ambiguous for dark slides; keeping the full screenshot is safer than
/// shipping black bars or cropping into the slide.
fn has_chrome_border(planes: &Planes, rect: Rect) -> bool {
    let band = (((rect.width().min(rect.height())) as f64 * 0.01) as usize).max(2);
    let chrome_line = |column: bool, position: usize, start: usize, end: usize| -> bool {
        if start >= end || position >= if column { planes.width } else { planes.height } {
            return false;
        }
        let step = ((end - start) / 80).max(1);
        let mut values = Vec::with_capacity((end - start) / step + 1);
        let mut saturation_sum = 0.0;
        let mut along = start;
        while along < end {
            let (x, y) = if column {
                (position, along)
            } else {
                (along, position)
            };
            values.push(planes.luma_at(x, y) as f64);
            saturation_sum += planes.saturation_at(x, y) as f64;
            along += step;
        }
        let count = values.len() as f64;
        let mean = values.iter().sum::<f64>() / count;
        let deviation = (values
            .iter()
            .map(|&value| (value - mean) * (value - mean))
            .sum::<f64>()
            / count)
            .sqrt();
        mean < 90.0 && saturation_sum / count < 30.0 && deviation < 16.0
    };
    let (x0, y0, x1, y1) = (
        rect.x0 as usize,
        rect.y0 as usize,
        rect.x1 as usize,
        rect.y1 as usize,
    );
    let top = (y0..(y0 + band).min(y1)).any(|row| chrome_line(false, row, x0, x1));
    let bottom = (y1.saturating_sub(band)..y1).any(|row| chrome_line(false, row, x0, x1));
    let left = (x0..(x0 + band).min(x1)).any(|column| chrome_line(true, column, y0, y1));
    let right = (x1.saturating_sub(band)..x1).any(|column| chrome_line(true, column, y0, y1));
    top || bottom || left || right
}

/// True when the excluded gap between two white runs looks like slide content
/// (a full-width banner or photo), not neutral chrome. Cropping such a gap away
/// would cut through the presentation itself.
fn gap_looks_like_content(planes: &Planes, rows: (usize, usize), columns: (usize, usize)) -> bool {
    if rows.0 >= rows.1 || columns.0 >= columns.1 {
        return false;
    }
    let step = ((columns.1 - columns.0) / 120).max(1);
    let mut values = Vec::new();
    let mut saturation_sum = 0.0;
    let mut y = rows.0;
    while y < rows.1 {
        let mut x = columns.0;
        while x < columns.1 {
            values.push(planes.luma_at(x, y) as f64);
            saturation_sum += planes.saturation_at(x, y) as f64;
            x += step;
        }
        y += 1;
    }
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let deviation = (values
        .iter()
        .map(|&value| (value - mean) * (value - mean))
        .sum::<f64>()
        / count)
        .sqrt();
    is_content(mean, saturation_sum / count, deviation)
}

/// True when an unmerged white-run gap (next to the chosen span) is content-like
/// and the span is nearly the full image width. This rejects cropping a
/// full-width slide that is split by a banner or photo, where no slide boundary
/// line exists to prove that the runs belong together.
fn has_wide_content_gap(
    planes: &Planes,
    runs: &[(usize, usize)],
    span: (usize, usize),
    columns: (usize, usize),
) -> bool {
    if ((columns.1 - columns.0) as f64) < planes.width as f64 * 0.8 {
        return false;
    }
    let minimum = ((planes.height as f64 * 0.02) as usize).max(2);
    let nearest_above = runs.iter().rev().find(|&&(_, end)| end <= span.0).copied();
    if let Some((_, end)) = nearest_above {
        if span.0 - end >= minimum && gap_looks_like_content(planes, (end, span.0), columns) {
            return true;
        }
    }
    let nearest_below = runs.iter().find(|&&(start, _)| start >= span.1).copied();
    if let Some((start, _)) = nearest_below {
        if start - span.1 >= minimum && gap_looks_like_content(planes, (span.1, start), columns) {
            return true;
        }
    }
    false
}

/// Detect a mostly white slide canvas and return its full-resolution bounds.
fn detect_white_canvas(image: &DynamicImage) -> Option<Rect> {
    let work = Planes::new(image, 640);
    let (width, height) = (work.width, work.height);
    let total = (width * height) as f64;
    let paper = work.luma_percentile(0.98);
    if paper < 160 {
        return None;
    }
    let threshold = paper.saturating_sub(6).clamp(205, 250);
    let white: Vec<bool> = work.luma.iter().map(|&value| value >= threshold).collect();
    let column_fraction = |x: usize, from: usize, to: usize| -> f64 {
        if from >= to {
            return 0.0;
        }
        let count = white
            .chunks_exact(width)
            .skip(from)
            .take(to - from)
            .filter(|row| row[x])
            .count();
        count as f64 / (to - from) as f64
    };
    let row_fraction = |y: usize, from: usize, to: usize| -> f64 {
        if from >= to {
            return 0.0;
        }
        white[y * width + from..y * width + to]
            .iter()
            .filter(|&&flag| flag)
            .count() as f64
            / (to - from) as f64
    };
    let gaps_x = ((width as f64 * 0.012) as usize).max(2);
    let gaps_y = ((height as f64 * 0.02) as usize).max(2);

    let columns: Vec<f64> = (0..width).map(|x| column_fraction(x, 0, height)).collect();
    let flags: Vec<bool> = columns.iter().map(|&value| value >= 0.22).collect();
    let (mut x0, mut x1) = bridge_runs(runs_of(&flags, 2), &columns, gaps_x, None, 8.0, 2)?;

    let mut y0 = 0;
    let mut y1 = 0;
    for _ in 0..2 {
        let band_rows: Vec<f64> = (0..height).map(|y| row_fraction(y, x0, x1)).collect();
        let flags: Vec<bool> = band_rows.iter().map(|&value| value >= 0.22).collect();
        let row_runs = runs_of(&flags, 2);
        let planes = &work;
        let (left_edge, right_edge) = (x0, x1);
        let continuation = move |gap_start: usize, gap_end: usize| -> f64 {
            let mut values = Vec::new();
            for y in gap_start..gap_end {
                for edge in [left_edge, right_edge] {
                    for offset in [-1i64, 0, 1] {
                        let x = (edge as i64 + offset).clamp(1, planes.width as i64 - 1) as usize;
                        values.push(planes.luma_at(x, y).abs_diff(planes.luma_at(x - 1, y)));
                    }
                }
            }
            percentile_u8(&mut values, 0.5) as f64
        };
        let merged = bridge_runs(
            row_runs.clone(),
            &band_rows,
            gaps_y,
            Some(&continuation),
            8.0,
            2,
        )?;
        if (x1 - x0) as f64 >= width as f64 * 0.8
            && has_wide_content_gap(&work, &row_runs, merged, (x0, x1))
        {
            return None;
        }
        y0 = merged.0;
        y1 = merged.1;

        let band_columns: Vec<f64> = (0..width).map(|x| column_fraction(x, y0, y1)).collect();
        let flags: Vec<bool> = band_columns.iter().map(|&value| value >= 0.22).collect();
        let merged = bridge_runs(runs_of(&flags, 2), &band_columns, gaps_x, None, 8.0, 2)?;
        x0 = merged.0;
        x1 = merged.1;
    }

    if y1 <= y0 {
        return None;
    }
    if ((x1 - x0) as f64) < width as f64 * 0.05 || ((y1 - y0) as f64) < height as f64 * 0.05 {
        return None;
    }
    if ((x1 - x0) * (y1 - y0)) as f64 > total * 0.985 {
        return None;
    }
    let inside: usize = (y0..y1)
        .map(|y| {
            white[y * width + x0..y * width + x1]
                .iter()
                .filter(|&&flag| flag)
                .count()
        })
        .sum();
    if (inside as f64 / ((x1 - x0) * (y1 - y0)) as f64) < 0.18 {
        return None;
    }

    let full = Planes::new(image, u32::MAX);
    let scale_x = full.width as f64 / width as f64;
    let scale_y = full.height as f64 / height as f64;
    let rough = Rect {
        x0: (x0 as f64 * scale_x) as u32,
        y0: (y0 as f64 * scale_y) as u32,
        x1: ((x1 as f64 * scale_x).round() as u32).min(full.width as u32),
        y1: ((y1 as f64 * scale_y).round() as u32).min(full.height as u32),
    };
    let refined = refine_edges(
        &full,
        expand_graphic_bands(&full, rough),
        (scale_x + scale_y) / 2.0,
        true,
    );

    if (refined.width() as f64) < full.width as f64 * 0.04
        || (refined.height() as f64) < full.height as f64 * 0.04
    {
        return None;
    }
    if (refined.width() as f64 * refined.height() as f64)
        > (full.width * full.height) as f64 * 0.985
    {
        return None;
    }
    let aspect = refined.width() as f64 / refined.height() as f64;
    if !(0.55..=3.2).contains(&aspect) {
        return None;
    }
    let mut sampled = Vec::new();
    let mut white_samples = 0usize;
    let mut y = refined.y0 as usize;
    while y < refined.y1 as usize {
        let mut x = refined.x0 as usize;
        while x < refined.x1 as usize {
            let value = full.luma_at(x, y);
            sampled.push(value);
            if value >= 244 {
                white_samples += 1;
            }
            x += 4;
        }
        y += 4;
    }
    if sampled.is_empty() {
        return None;
    }
    let sampled_paper = percentile_u8(&mut sampled, 0.98);
    if sampled_paper < 247 || (white_samples as f64 / sampled.len() as f64) < 0.10 {
        return None;
    }
    Some(refined)
}

/// Small RGB copy used by the edge fallback, which compares raw channel
/// differences rather than luma.
struct RgbWork {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl RgbWork {
    fn new(image: &DynamicImage, max_side: u32) -> Self {
        let (source_width, source_height) = image.dimensions();
        let scale = (max_side as f64 / source_width.max(source_height) as f64).min(1.0);
        let width = ((source_width as f64 * scale).round() as u32).max(1);
        let height = ((source_height as f64 * scale).round() as u32).max(1);
        let rgb = if width == source_width && height == source_height {
            image.to_rgb8()
        } else {
            image
                .resize_exact(width, height, FilterType::Lanczos3)
                .to_rgb8()
        };
        Self {
            width: width as usize,
            height: height as usize,
            data: rgb.into_raw(),
        }
    }

    #[inline]
    fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let index = (y * self.width + x) * 3;
        [self.data[index], self.data[index + 1], self.data[index + 2]]
    }

    #[inline]
    fn luma(&self, x: usize, y: usize) -> f64 {
        let [red, green, blue] = self.pixel(x, y);
        0.299 * red as f64 + 0.587 * green as f64 + 0.114 * blue as f64
    }

    fn difference(&self, from: (usize, usize), to: (usize, usize)) -> u32 {
        let [red_a, green_a, blue_a] = self.pixel(from.0, from.1);
        let [red_b, green_b, blue_b] = self.pixel(to.0, to.1);
        (red_a.abs_diff(red_b) as u32
            + green_a.abs_diff(green_b) as u32
            + blue_a.abs_diff(blue_b) as u32)
            / 3
    }
}

/// Conservative fallback for dark full-bleed decks. Finds the strongest
/// standard-ratio rectangle bounded by edge transitions, then rejects results
/// that still contain chrome-like bands.
#[allow(clippy::needless_range_loop)]
fn detect_edge_fallback(image: &DynamicImage) -> Option<Rect> {
    let work = RgbWork::new(image, 480);
    let (width, height) = (work.width, work.height);
    if width < 16 || height < 16 {
        return None;
    }
    let vertical_stride = height + 1;
    let mut vertical = vec![0u32; width * vertical_stride];
    for x in 1..width {
        for y in 0..height {
            let previous = vertical[x * vertical_stride + y];
            vertical[x * vertical_stride + y + 1] = previous + work.difference((x, y), (x - 1, y));
        }
    }
    let horizontal_stride = width + 1;
    let mut horizontal = vec![0u32; height * horizontal_stride];
    for y in 1..height {
        for x in 0..width {
            let previous = horizontal[y * horizontal_stride + x];
            horizontal[y * horizontal_stride + x + 1] =
                previous + work.difference((x, y), (x, y - 1));
        }
    }
    let candidates = |prefix: &[u32], count: usize, stride: usize| -> Vec<usize> {
        let mut edges: Vec<usize> = (1..count).collect();
        edges.sort_by_key(|&edge| std::cmp::Reverse(prefix[edge * stride + stride - 1]));
        let minimum_gap = ((count as f64 * 0.005) as usize).max(2);
        let mut selected = Vec::new();
        for edge in edges {
            if prefix[edge * stride + stride - 1] == 0 {
                break;
            }
            if selected
                .iter()
                .all(|&old: &usize| old.abs_diff(edge) > minimum_gap)
            {
                selected.push(edge);
                if selected.len() == 24 {
                    break;
                }
            }
        }
        selected.sort_unstable();
        selected
    };
    let xs = candidates(&vertical, width, vertical_stride);
    let ys = candidates(&horizontal, height, horizontal_stride);
    let coverage = |prefix: &[u32], edge: usize, start: usize, end: usize, stride: usize| -> f64 {
        (prefix[edge * stride + end] - prefix[edge * stride + start]) as f64 / (end - start) as f64
    };
    let band_mean = |xa: usize, xb: usize, ya: usize, yb: usize| -> f64 {
        if xa >= xb || ya >= yb {
            return 0.0;
        }
        let step_x = ((xb - xa) / 80).max(1);
        let step_y = ((yb - ya) / 80).max(1);
        let mut sum = 0.0;
        let mut count = 0;
        let mut y = ya;
        while y < yb {
            let mut x = xa;
            while x < xb {
                sum += work.luma(x, y);
                count += 1;
                x += step_x;
            }
            y += step_y;
        }
        sum / count.max(1) as f64
    };
    let ratio_error = |aspect: f64| -> f64 {
        [16.0 / 9.0, 4.0 / 3.0, 16.0 / 10.0]
            .iter()
            .map(|standard| (aspect / standard - 1.0).abs())
            .fold(f64::INFINITY, f64::min)
    };
    let mut best: Option<(f64, Rect)> = None;
    for (index, &x0) in xs.iter().enumerate() {
        for &x1 in &xs[index + 1..] {
            let slide_width = x1 - x0;
            if (slide_width as f64) < width as f64 * 0.12 {
                continue;
            }
            for (second, &y0) in ys.iter().enumerate() {
                for &y1 in &ys[second + 1..] {
                    let slide_height = y1 - y0;
                    if (slide_height as f64) < height as f64 * 0.12 {
                        continue;
                    }
                    let area = (slide_width * slide_height) as f64;
                    if area < (width * height) as f64 * 0.06 {
                        continue;
                    }
                    let aspect = slide_width as f64 / slide_height as f64;
                    if !(1.0..=2.8).contains(&aspect) {
                        continue;
                    }
                    let strength = [
                        coverage(&vertical, x0, y0, y1, vertical_stride),
                        coverage(&vertical, x1, y0, y1, vertical_stride),
                        coverage(&horizontal, y0, x0, x1, horizontal_stride),
                        coverage(&horizontal, y1, x0, x1, horizontal_stride),
                    ]
                    .into_iter()
                    .fold(f64::INFINITY, f64::min);
                    if strength < 18.0 {
                        continue;
                    }
                    let margin = (((slide_width.min(slide_height)) as f64 * 0.03) as usize).max(1);
                    if x0 + margin >= x1 - margin || y0 + margin >= y1 - margin {
                        continue;
                    }
                    let inside = band_mean(x0 + margin, x1 - margin, y0 + margin, y1 - margin);
                    let mut outside = 0.0;
                    let mut outside_bands = 0;
                    if x0 >= margin {
                        outside += band_mean(x0 - margin, x0, y0 + margin, y1 - margin);
                        outside_bands += 1;
                    }
                    if x1 + margin <= width {
                        outside += band_mean(x1, x1 + margin, y0 + margin, y1 - margin);
                        outside_bands += 1;
                    }
                    if y0 >= margin {
                        outside += band_mean(x0 + margin, x1 - margin, y0 - margin, y0);
                        outside_bands += 1;
                    }
                    if y1 + margin <= height {
                        outside += band_mean(x0 + margin, x1 - margin, y1, y1 + margin);
                        outside_bands += 1;
                    }
                    if outside_bands == 0 {
                        continue;
                    }
                    if (inside - outside / outside_bands as f64).abs() < 28.0 {
                        continue;
                    }
                    if ratio_error(aspect) > 0.06 {
                        continue;
                    }
                    let score = area * (strength / 30.0);
                    if best.is_none_or(|(current, _)| score > current) {
                        best = Some((
                            score,
                            Rect {
                                x0: x0 as u32,
                                y0: y0 as u32,
                                x1: x1 as u32,
                                y1: y1 as u32,
                            },
                        ));
                    }
                }
            }
        }
    }
    let (_, best) = best?;
    let full = Planes::new(image, u32::MAX);
    let scale_x = full.width as f64 / width as f64;
    let scale_y = full.height as f64 / height as f64;
    let rough = Rect {
        x0: (best.x0 as f64 * scale_x) as u32,
        y0: (best.y0 as f64 * scale_y) as u32,
        x1: (best.x1 as f64 * scale_x) as u32,
        y1: (best.y1 as f64 * scale_y) as u32,
    };
    let refined = refine_edges(&full, rough, (scale_x + scale_y) / 2.0, false);
    if has_chrome_border(&full, refined) {
        None
    } else {
        Some(refined)
    }
}

pub(super) fn detect_presentation(image: &DynamicImage) -> Option<(u32, u32, u32, u32)> {
    if image.width() < 100 || image.height() < 100 {
        return None;
    }
    let probe = Planes::new(image, 320);
    let bright =
        probe.luma.iter().filter(|&&value| value >= 235).count() as f64 / probe.luma.len() as f64;
    let rect = if bright < 0.02 {
        detect_edge_fallback(image)
    } else if let Some(rect) = detect_white_canvas(image) {
        Some(rect)
    } else {
        let left = probe.width / 3;
        let top = probe.height / 3;
        let mut sum = 0u64;
        let mut count = 0u64;
        let mut y = top;
        while y < probe.height - top {
            let mut x = left;
            while x < probe.width - left {
                sum += probe.luma_at(x, y) as u64;
                count += 1;
                x += 2;
            }
            y += 2;
        }
        let centre = sum as f64 / count.max(1) as f64;
        let strict_white = probe.luma.iter().filter(|&&value| value >= 244).count() as f64
            / probe.luma.len() as f64;
        // A dark deck on light chrome can defeat white detection. Only retry
        // the edge fallback when the centre is dark and the picture is not
        // dominated by strict white (which would indicate a bright slide whose
        // centre merely contains a dark photo).
        if centre < 110.0 && strict_white < 0.30 {
            detect_edge_fallback(image)
        } else {
            None
        }
    };
    rect.map(|rect| (rect.x0, rect.y0, rect.width(), rect.height()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [u8; 3] = [255, 255, 255];
    const BLACK: [u8; 3] = [0, 0, 0];
    const DARK_CHROME: [u8; 3] = [34, 34, 34];
    const PANEL: [u8; 3] = [28, 28, 28];

    fn canvas(width: u32, height: u32, color: [u8; 3]) -> image::RgbImage {
        image::RgbImage::from_pixel(width, height, image::Rgb(color))
    }

    fn fill(image: &mut image::RgbImage, x0: i64, y0: i64, x1: i64, y1: i64, color: [u8; 3]) {
        let width = image.width() as i64;
        let height = image.height() as i64;
        for y in y0.max(0)..y1.min(height) {
            for x in x0.max(0)..x1.min(width) {
                image.put_pixel(x as u32, y as u32, image::Rgb(color));
            }
        }
    }

    fn detect(image: &image::RgbImage) -> Option<(u32, u32, u32, u32)> {
        detect_presentation(&DynamicImage::ImageRgb8(image.clone()))
    }

    fn assert_crop(
        detected: Option<(u32, u32, u32, u32)>,
        expected: (u32, u32, u32, u32),
        tolerance: u32,
    ) {
        let (x, y, width, height) = detected.expect("presentation area was not detected");
        let actual = (x, y, x + width, y + height);
        for (found, wanted) in [
            (actual.0, expected.0),
            (actual.1, expected.1),
            (actual.2, expected.2),
            (actual.3, expected.3),
        ] {
            assert!(
                (found as i64 - wanted as i64).abs() <= tolerance as i64,
                "crop {actual:?} differs from expected {expected:?} beyond {tolerance}px"
            );
        }
    }

    fn fit_16_9(area: (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
        let (x0, y0, x1, y1) = area;
        let (width, height) = (x1 - x0, y1 - y0);
        let (slide_width, slide_height) = if width as f64 / height as f64 > 16.0 / 9.0 {
            ((height as f64 * 16.0 / 9.0).round() as u32, height)
        } else {
            (width, (width as f64 * 9.0 / 16.0).round() as u32)
        };
        let x = x0 + (width - slide_width) / 2;
        let y = y0 + (height - slide_height) / 2;
        (x, y, x + slide_width, y + slide_height)
    }

    /// Full-window meeting app with a dark toolbar, side panel, control bar,
    /// black letterbox, and a centered slide.
    fn dark_meeting_screenshot() -> (image::RgbImage, (u32, u32, u32, u32)) {
        let (width, height) = (1200, 675);
        let mut image = canvas(width, height, DARK_CHROME);
        fill(&mut image, 0, 0, width as i64, 40, PANEL);
        fill(&mut image, 0, 40, 220, height as i64 - 60, PANEL);
        fill(
            &mut image,
            0,
            height as i64 - 60,
            width as i64,
            height as i64,
            PANEL,
        );
        fill(&mut image, 220, 40, width as i64, height as i64 - 60, BLACK);
        let slide = fit_16_9((220, 40, width, height - 60));
        fill(
            &mut image,
            slide.0 as i64,
            slide.1 as i64,
            slide.2 as i64,
            slide.3 as i64,
            WHITE,
        );
        (image, slide)
    }

    #[test]
    fn crops_the_white_slide_inside_dark_meeting_chrome() {
        let (image, slide) = dark_meeting_screenshot();
        assert_crop(detect(&image), slide, 2);
    }

    #[test]
    fn crops_when_the_meeting_chrome_and_taskbar_are_light() {
        let (width, height) = (1200, 675);
        let mut image = canvas(width, height, [58, 110, 165]);
        fill(
            &mut image,
            0,
            0,
            width as i64,
            height as i64,
            [240, 240, 240],
        );
        fill(&mut image, 0, 0, width as i64, 40, [232, 232, 232]);
        fill(&mut image, 0, 40, 220, height as i64 - 60, [246, 246, 246]);
        fill(
            &mut image,
            0,
            height as i64 - 60,
            width as i64,
            height as i64,
            [232, 232, 232],
        );
        fill(
            &mut image,
            220,
            40,
            width as i64,
            height as i64 - 60,
            [224, 224, 224],
        );
        let slide = fit_16_9((220, 40, width, height - 60));
        fill(
            &mut image,
            slide.0 as i64 - 2,
            slide.1 as i64 - 2,
            slide.2 as i64 + 2,
            slide.3 as i64 + 2,
            [200, 200, 200],
        );
        fill(
            &mut image,
            slide.0 as i64,
            slide.1 as i64,
            slide.2 as i64,
            slide.3 as i64,
            WHITE,
        );
        fill(
            &mut image,
            0,
            height as i64 - 28,
            width as i64,
            height as i64,
            [240, 240, 240],
        );
        assert_crop(detect(&image), slide, 2);
    }

    #[test]
    fn keeps_a_colored_title_band_at_the_top_of_the_slide() {
        let (mut image, slide) = dark_meeting_screenshot();
        let band = (slide.3 - slide.1) * 22 / 100;
        fill(
            &mut image,
            slide.0 as i64,
            slide.1 as i64,
            slide.2 as i64,
            (slide.1 + band) as i64,
            [16, 48, 120],
        );
        assert_crop(detect(&image), slide, 2);
    }

    #[test]
    fn keeps_a_photo_that_touches_the_bottom_of_the_slide() {
        let (mut image, slide) = dark_meeting_screenshot();
        let photo_top = slide.1 + (slide.3 - slide.1) * 55 / 100;
        fill(
            &mut image,
            slide.0 as i64 + 4,
            photo_top as i64,
            slide.2 as i64 - 4,
            slide.3 as i64,
            [15, 20, 35],
        );
        assert_crop(detect(&image), slide, 2);
    }

    #[test]
    fn prefers_the_slide_over_a_small_white_neighbour_window() {
        let (mut image, slide) = dark_meeting_screenshot();
        fill(&mut image, 10, 60, 150, 500, WHITE);
        assert_crop(detect(&image), slide, 2);
    }

    #[test]
    fn crops_a_portrait_document_to_its_actual_aspect_ratio() {
        let (width, height) = (1200, 900);
        let mut image = canvas(width, height, [8, 8, 10]);
        let document = (350, 100, 850, 800);
        fill(
            &mut image,
            document.0 as i64,
            document.1 as i64,
            document.2 as i64,
            document.3 as i64,
            WHITE,
        );
        assert_crop(detect(&image), document, 2);
    }

    #[test]
    fn crops_a_dark_slide_surrounded_by_light_neutral_chrome() {
        let (width, height) = (1200, 675);
        let mut image = canvas(width, height, [240, 240, 240]);
        let slide = (80, 45, 1120, 630);
        fill(
            &mut image,
            slide.0 as i64,
            slide.1 as i64,
            slide.2 as i64,
            slide.3 as i64,
            [24, 28, 40],
        );
        for bar in 0..8 {
            fill(
                &mut image,
                (slide.0 + 60 + bar * 110) as i64,
                (slide.3 - 160) as i64,
                (slide.0 + 120 + bar * 110) as i64,
                (slide.3 - 100) as i64,
                [0, 140, 210],
            );
        }
        assert_crop(detect(&image), slide, 3);
    }

    #[test]
    fn does_not_crop_a_uniform_or_full_screen_slide() {
        let uniform = canvas(400, 300, [230, 230, 230]);
        assert!(detect(&uniform).is_none());

        let (width, height) = (1200, 675);
        let mut full_screen = canvas(width, height, WHITE);
        fill(&mut full_screen, 60, 60, 600, 90, [30, 30, 40]);
        for bar in 0..6 {
            fill(
                &mut full_screen,
                80 + bar * 150,
                480,
                160 + bar * 150,
                600,
                [0, 112, 192],
            );
        }
        assert!(detect(&full_screen).is_none());
    }

    #[test]
    fn tolerates_compression_noise_around_the_white_canvas() {
        let (mut image, slide) = dark_meeting_screenshot();
        let mut seed = 0x9e37_79b9u32;
        for y in 0..image.height() {
            for x in 0..image.width() {
                let pixel = image.get_pixel(x, y);
                let value = pixel.0[0];
                if value == 255 || value == 0 || value == 34 || value == 28 {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let noise = ((seed >> 16) % 5) as i32 - 2;
                    let adjusted = (value as i32 + noise).clamp(0, 255) as u8;
                    image.put_pixel(x, y, image::Rgb([adjusted, adjusted, adjusted]));
                }
            }
        }
        assert_crop(detect(&image), slide, 2);
    }
}
