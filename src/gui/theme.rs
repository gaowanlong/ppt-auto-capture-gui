//! Shared visual language: a quiet workspace with pine accents and a single
//! recording-red signal, available in a light and a dark palette.
//!
//! Colours are not constants: the palette is chosen at runtime (and can change
//! while the application runs, see the appearance page), so every colour is read
//! through [`token`]. Panels therefore ask for a role — `token::surface()`,
//! `token::ink()` — instead of a baked-in value.

use egui::{
    self, Align2, Button, Color32, CornerRadius, CursorIcon, DragValue, FontFamily, FontId, Frame,
    Key, Margin, Response, RichText, Sense, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui, Vec2,
    WidgetText,
};
use std::ops::RangeInclusive;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    RwLock,
};

use crate::i18n::{self, Language};

/// Every colour the interface uses, in one place. Both palettes share the field
/// names, so a panel names the role it needs and gets the colour for the active
/// mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Page background behind the cards.
    pub bg: Color32,
    pub sidebar: Color32,
    pub sidebar_hover: Color32,
    /// The row of the page the user is on. It has to lift off the sidebar in
    /// both palettes, so it is not simply [`Self::surface`].
    pub nav_selected: Color32,
    /// Card and popup surface.
    pub surface: Color32,
    /// Recessed surface: secondary buttons, paths, value fields.
    pub surface_soft: Color32,
    /// Primary text.
    pub ink: Color32,
    /// Secondary text and labels.
    pub muted: Color32,
    /// Metadata that should recede.
    pub faint: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    /// Filled controls and indicators.
    pub primary: Color32,
    /// The same pine as text or icon stroke, kept readable on dark surfaces.
    pub accent: Color32,
    pub primary_soft: Color32,
    /// Text drawn on top of [`Self::primary`].
    pub on_primary: Color32,
    pub record: Color32,
    pub record_soft: Color32,
    pub record_line: Color32,
    pub warn: Color32,
    pub warn_soft: Color32,
    pub warn_line: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub danger_line: Color32,
    pub ok: Color32,
    pub ok_soft: Color32,
}

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xF3, 0xF6, 0xF4),
    sidebar: Color32::from_rgb(0xEA, 0xEF, 0xEC),
    sidebar_hover: Color32::from_rgb(0xE1, 0xE7, 0xE3),
    nav_selected: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface_soft: Color32::from_rgb(0xF1, 0xF4, 0xF2),
    ink: Color32::from_rgb(0x1C, 0x2C, 0x32),
    muted: Color32::from_rgb(0x5F, 0x6F, 0x74),
    faint: Color32::from_rgb(0x81, 0x8F, 0x8B),
    line: Color32::from_rgb(0xDC, 0xE3, 0xDE),
    line_strong: Color32::from_rgb(0xC3, 0xCE, 0xC8),
    primary: Color32::from_rgb(0x27, 0x4F, 0x46),
    accent: Color32::from_rgb(0x27, 0x4F, 0x46),
    primary_soft: Color32::from_rgb(0xE4, 0xEE, 0xE9),
    on_primary: Color32::WHITE,
    record: Color32::from_rgb(0xBE, 0x47, 0x3C),
    record_soft: Color32::from_rgb(0xFB, 0xF0, 0xED),
    record_line: Color32::from_rgb(0xEC, 0xD0, 0xCA),
    warn: Color32::from_rgb(0x8F, 0x65, 0x1E),
    warn_soft: Color32::from_rgb(0xFA, 0xF3, 0xE4),
    warn_line: Color32::from_rgb(0xE8, 0xD7, 0xB4),
    danger: Color32::from_rgb(0xA9, 0x39, 0x31),
    danger_soft: Color32::from_rgb(0xFA, 0xEE, 0xEC),
    danger_line: Color32::from_rgb(0xE9, 0xC9, 0xC5),
    ok: Color32::from_rgb(0x2E, 0x6B, 0x50),
    ok_soft: Color32::from_rgb(0xE8, 0xF2, 0xEC),
};

/// A slate-green dark mode: the same roles, inverted, with the pine accent
/// lightened so it reads as text and keeps white readable as a button label.
pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x10, 0x17, 0x16),
    sidebar: Color32::from_rgb(0x15, 0x1D, 0x1C),
    sidebar_hover: Color32::from_rgb(0x1F, 0x29, 0x27),
    nav_selected: Color32::from_rgb(0x27, 0x33, 0x30),
    surface: Color32::from_rgb(0x1B, 0x24, 0x23),
    surface_soft: Color32::from_rgb(0x23, 0x2D, 0x2C),
    ink: Color32::from_rgb(0xE8, 0xEF, 0xEC),
    muted: Color32::from_rgb(0xA0, 0xAD, 0xA9),
    faint: Color32::from_rgb(0x7C, 0x89, 0x85),
    line: Color32::from_rgb(0x2C, 0x37, 0x35),
    line_strong: Color32::from_rgb(0x3E, 0x4A, 0x48),
    primary: Color32::from_rgb(0x2F, 0x6B, 0x58),
    accent: Color32::from_rgb(0x7F, 0xD3, 0xB2),
    primary_soft: Color32::from_rgb(0x1F, 0x33, 0x2C),
    on_primary: Color32::WHITE,
    record: Color32::from_rgb(0xE4, 0x70, 0x5F),
    record_soft: Color32::from_rgb(0x2E, 0x1C, 0x19),
    record_line: Color32::from_rgb(0x4B, 0x2B, 0x26),
    warn: Color32::from_rgb(0xDC, 0xA9, 0x5A),
    warn_soft: Color32::from_rgb(0x2E, 0x26, 0x17),
    warn_line: Color32::from_rgb(0x4A, 0x3B, 0x21),
    danger: Color32::from_rgb(0xE6, 0x7C, 0x70),
    danger_soft: Color32::from_rgb(0x2F, 0x1B, 0x19),
    danger_line: Color32::from_rgb(0x4E, 0x2B, 0x27),
    ok: Color32::from_rgb(0x6F, 0xC7, 0x9A),
    ok_soft: Color32::from_rgb(0x18, 0x2A, 0x20),
};

static ACTIVE: RwLock<Palette> = RwLock::new(LIGHT);
static DARK_MODE: AtomicBool = AtomicBool::new(false);

pub fn palette() -> Palette {
    *ACTIVE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Whether the active palette is the dark one.
pub fn is_dark() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

/// The colour roles, resolved against the active palette.
pub mod token {
    use super::palette;
    use egui::Color32;

    macro_rules! palette_tokens {
        ($($name:ident),* $(,)?) => {
            $(
                #[inline]
                pub fn $name() -> Color32 {
                    palette().$name
                }
            )*
        };
    }

    palette_tokens!(
        bg,
        sidebar,
        sidebar_hover,
        nav_selected,
        surface,
        surface_soft,
        ink,
        muted,
        faint,
        line,
        line_strong,
        primary,
        accent,
        primary_soft,
        on_primary,
        record,
        record_soft,
        record_line,
        warn,
        warn_soft,
        warn_line,
        danger,
        danger_soft,
        danger_line,
        ok,
        ok_soft,
    );
}

/// Install a palette and rebuild the widget style. Called at startup and
/// whenever the light/dark choice changes.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let palette = if dark { DARK } else { LIGHT };
    *ACTIVE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = palette;
    DARK_MODE.store(dark, Ordering::Relaxed);

    let style = build_style(palette, dark);
    // Both theme slots get the same style. egui keeps one style per theme, so
    // setting only the active one would fall back to egui's own colours if the
    // system theme changed while the application runs.
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

fn build_style(palette: Palette, dark: bool) -> egui::Style {
    let mut style = egui::Style {
        text_styles: [
            (
                TextStyle::Heading,
                FontId::new(26.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(11.5, FontFamily::Monospace),
            ),
        ]
        .into(),
        ..Default::default()
    };

    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.surface;
    visuals.window_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.extreme_bg_color = palette.surface_soft;
    visuals.faint_bg_color = palette.surface_soft;
    visuals.code_bg_color = palette.surface_soft;
    visuals.selection = egui::style::Selection {
        bg_fill: palette.primary_soft,
        stroke: Stroke::new(1.0_f32, palette.primary),
    };
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warn;
    visuals.error_fg_color = palette.danger;
    visuals.button_frame = true;
    visuals.striped = false;
    visuals.slider_trailing_fill = true;
    visuals.override_text_color = Some(palette.ink);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 96 } else { 36 }),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 22,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 88 } else { 32 }),
    };

    let radius = CornerRadius::same(4);
    visuals.widgets.noninteractive = egui::style::WidgetVisuals {
        bg_fill: palette.surface,
        weak_bg_fill: palette.surface,
        bg_stroke: Stroke::new(1.0_f32, palette.line),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0_f32, palette.ink),
        expansion: 0.0,
    };
    visuals.widgets.inactive = egui::style::WidgetVisuals {
        bg_fill: palette.surface,
        weak_bg_fill: palette.surface_soft,
        bg_stroke: Stroke::new(1.0_f32, palette.line),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0_f32, palette.ink),
        expansion: 0.0,
    };
    visuals.widgets.hovered = egui::style::WidgetVisuals {
        bg_fill: palette.surface,
        weak_bg_fill: palette.sidebar_hover,
        bg_stroke: Stroke::new(1.0_f32, palette.line_strong),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0_f32, palette.ink),
        expansion: 0.0,
    };
    visuals.widgets.active = egui::style::WidgetVisuals {
        bg_fill: palette.primary_soft,
        weak_bg_fill: palette.primary_soft,
        bg_stroke: Stroke::new(1.0_f32, palette.primary),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0_f32, palette.accent),
        expansion: 0.0,
    };
    visuals.widgets.open = visuals.widgets.hovered;
    style.visuals = visuals;

    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(14.0, 7.0);
    style.spacing.interact_size = Vec2::new(36.0, 34.0);
    style.spacing.slider_width = 210.0;
    style.spacing.text_edit_width = 360.0;
    style.spacing.combo_width = 108.0;
    style.spacing.indent = 18.0;
    style.spacing.window_margin = Margin::same(18);
    style.spacing.menu_margin = Margin::same(10);
    style.spacing.tooltip_width = 340.0;
    style
}

/// Draw `texture` over the whole workspace, then fade it by `dim` percent.
///
/// The picture keeps its aspect ratio and is scaled to cover the area, so it is
/// never stretched or letterboxed; the parts outside the visible area are
/// clipped by the caller's clip rectangle.
pub fn paint_background(ui: &Ui, texture: &egui::TextureHandle, dim: f64) {
    let canvas = ui.clip_rect();
    let source = texture.size_vec2();
    if canvas.width() < 1.0 || canvas.height() < 1.0 || source.x < 1.0 || source.y < 1.0 {
        return;
    }
    let scale = (canvas.width() / source.x).max(canvas.height() / source.y);
    let target = egui::Rect::from_center_size(canvas.center(), source * scale);
    let painter = ui.painter();
    painter.image(
        texture.id(),
        target,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    // The scrim keeps the interface readable whatever picture was chosen.
    let alpha = ((dim.clamp(0.0, 90.0) / 100.0) * 255.0).round() as u8;
    if alpha > 0 {
        let bg = token::bg();
        painter.rect_filled(
            canvas,
            0.0,
            Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), alpha),
        );
    }
}

pub fn card() -> Frame {
    Frame::new()
        .fill(token::surface())
        .stroke(Stroke::new(1.0_f32, token::line()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(20, 18))
}

pub fn banner_frame(fill: Color32, stroke: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, stroke))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(16, 12))
}

pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(26.0).color(token::ink()).strong());
    ui.add_space(3.0);
    ui.label(RichText::new(subtitle).size(13.0).color(token::muted()));
    ui.add_space(20.0);
}

pub fn section_title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(12.0)
            .color(token::muted())
            .strong(),
    );
}

pub fn muted(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).size(size).color(token::muted())
}

pub fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .monospace()
        .size(11.0)
        .color(token::muted())
}

pub fn divider(ui: &mut Ui) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0_f32, token::line()),
    );
    ui.add_space(10.0);
}

pub fn primary_button(text: impl Into<String>) -> Button<'static> {
    Button::new(
        RichText::new(text.into())
            .size(14.0)
            .color(token::on_primary())
            .strong(),
    )
    .fill(token::primary())
    .stroke(Stroke::NONE)
    .corner_radius(8)
    .min_size(Vec2::new(0.0, 38.0))
}

pub fn secondary_button(text: impl Into<String>) -> Button<'static> {
    Button::new(RichText::new(text.into()).size(13.5).color(token::ink()))
        .fill(token::surface_soft())
        .stroke(Stroke::new(1.0_f32, token::line()))
        .corner_radius(8)
        .min_size(Vec2::new(0.0, 36.0))
}

/// One option of a segmented control. Selected and unselected options keep the
/// same font, stroke, and height, so switching choices never makes the row jump.
pub fn segment_button(text: impl Into<String>, selected: bool) -> Button<'static> {
    let text = text.into();
    let (fill, stroke, color) = if selected {
        (token::primary(), token::primary(), token::on_primary())
    } else {
        (token::surface_soft(), token::line(), token::ink())
    };
    Button::new(RichText::new(text).size(13.5).color(color))
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, stroke))
        .corner_radius(8)
        .min_size(Vec2::new(0.0, 36.0))
}

pub fn danger_button(text: impl Into<String>) -> Button<'static> {
    Button::new(RichText::new(text.into()).size(13.5).color(token::danger()))
        .fill(token::danger_soft())
        .stroke(Stroke::new(1.0_f32, token::danger_line()))
        .corner_radius(8)
        .min_size(Vec2::new(0.0, 36.0))
}

pub fn quiet_button(text: impl Into<String>) -> Button<'static> {
    Button::new(RichText::new(text.into()).size(12.0).color(token::accent())).frame(false)
}

pub fn empty_note(ui: &mut Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(muted(text, 12.0));
    ui.add_space(8.0);
}

/// A quiet bullet-free note row, used for explanatory lists.
pub fn note_row(ui: &mut Ui, text: &str) {
    let text = text.trim_start_matches(['•', '·', '-', ' ']).trim();
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(13.0, 18.0), Sense::hover());
        ui.painter().circle_filled(
            egui::pos2(rect.left() + 3.0, rect.center().y),
            1.8,
            token::faint(),
        );
        ui.label(RichText::new(text).size(12.5).color(token::muted()));
    });
}

/// A soft tinted strip used for paths and short summaries.
pub fn soft_frame() -> Frame {
    Frame::new()
        .fill(token::surface_soft())
        .stroke(Stroke::new(1.0_f32, token::line()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 9))
}

/// A slim pine slider followed by a fixed-width value field. Unlike egui's
/// built-in slider, the value box keeps the same width for every row, so
/// neighbouring settings line up even when their ranges differ in digits.
pub fn slider(
    ui: &mut Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    step: f64,
    decimals: usize,
    suffix: &str,
) -> Response {
    let mut changed = false;
    let row = ui.horizontal(|ui| {
        let box_width = 92.0;
        let track_width =
            (ui.available_width() - box_width - ui.spacing().item_spacing.x).max(120.0);
        let track = slider_track(ui, value, &range, step, track_width);
        changed |= track.changed();
        let field = ui.add_sized(
            [box_width, 26.0],
            DragValue::new(value)
                .range(range)
                .speed(step)
                .min_decimals(decimals)
                .max_decimals(decimals)
                .suffix(suffix),
        );
        changed |= field.changed();
        track.union(field)
    });
    let mut response = row.inner;
    if changed {
        response.mark_changed();
    }
    response
}

fn slider_track(
    ui: &mut Ui,
    value: &mut f64,
    range: &RangeInclusive<f64>,
    step: f64,
    width: f32,
) -> Response {
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click_and_drag());
    let handle_radius = 7.5;
    let x0 = rect.left() + handle_radius;
    let x1 = rect.right() - handle_radius;
    let span = (x1 - x0).max(1.0);
    let start = *range.start();
    let end = *range.end();
    let mut next = *value;

    if let Some(pointer) = response.interact_pointer_pos() {
        let fraction = ((pointer.x - x0) / span).clamp(0.0, 1.0) as f64;
        let raw = start + fraction * (end - start);
        next = (start + ((raw - start) / step).round() * step).clamp(start, end);
    }
    if response.has_focus() || response.clicked() {
        let (left, right) = ui.input(|input| {
            (
                input.num_presses(Key::ArrowLeft),
                input.num_presses(Key::ArrowRight),
            )
        });
        if right > 0 || left > 0 {
            next = (*value + step * (right as f64 - left as f64)).clamp(start, end);
        }
    }
    if response.clicked() || response.dragged() {
        response.request_focus();
    }
    if (next - *value).abs() > f64::EPSILON {
        *value = next;
        response.mark_changed();
    }

    let fraction = ((*value - start) / (end - start)).clamp(0.0, 1.0) as f32;
    let center = egui::pos2(x0 + fraction * span, rect.center().y);
    let painter = ui.painter();
    let rail = egui::Rect::from_min_max(
        egui::pos2(x0, rect.center().y - 2.5),
        egui::pos2(x1, rect.center().y + 2.5),
    );
    painter.rect_filled(rail, CornerRadius::same(3), token::line());
    painter.rect_filled(
        egui::Rect::from_min_max(rail.min, egui::pos2(center.x, rail.max.y)),
        CornerRadius::same(3),
        token::primary(),
    );
    let active = response.hovered() || response.dragged() || response.has_focus();
    if response.has_focus() && !response.dragged() {
        painter.circle_stroke(center, 11.5, Stroke::new(1.0_f32, token::primary_soft()));
    }
    painter.circle(
        center,
        if active { 8.5 } else { 7.5 },
        token::surface(),
        Stroke::new(2.0_f32, token::accent()),
    );
    response
}

#[derive(Clone, Copy)]
pub enum NavIcon {
    Overview,
    Window,
    Display,
    Settings,
    Output,
    Create,
    /// A half-filled disc, the usual "contrast" mark for appearance settings.
    Theme,
}

pub fn nav_item(ui: &mut Ui, selected: bool, icon: NavIcon, label: &str) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(9), token::nav_selected());
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.left_top() + Vec2::new(0.0, 10.0),
                Vec2::new(3.0, rect.height() - 20.0),
            ),
            CornerRadius::same(2),
            token::accent(),
        );
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(9), token::sidebar_hover());
    }

    let foreground = if selected {
        token::accent()
    } else {
        token::muted()
    };
    let icon_center = egui::pos2(rect.left() + 22.0, rect.center().y);
    paint_icon(painter, icon, icon_center, foreground);
    painter.text(
        egui::pos2(rect.left() + 42.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, FontFamily::Proportional),
        foreground,
    );
    response
}

fn paint_icon(painter: &egui::Painter, icon: NavIcon, center: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let p = |dx: f32, dy: f32| center + Vec2::new(dx, dy);
    match icon {
        NavIcon::Overview => {
            for (x, y) in [(-4.5, -4.5), (4.5, -4.5), (-4.5, 4.5), (4.5, 4.5)] {
                let r = egui::Rect::from_center_size(p(x, y), Vec2::splat(6.0));
                painter.rect_stroke(r, CornerRadius::same(1), stroke, StrokeKind::Inside);
            }
        }
        NavIcon::Window => {
            let r = egui::Rect::from_center_size(p(0.0, -1.0), Vec2::new(15.0, 12.0));
            painter.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
            painter.line_segment([p(-7.5, -3.0), p(7.5, -3.0)], stroke);
        }
        NavIcon::Display => {
            let r = egui::Rect::from_center_size(p(0.0, -2.0), Vec2::new(15.0, 10.0));
            painter.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
            painter.line_segment([p(0.0, 3.0), p(0.0, 6.0)], stroke);
            painter.line_segment([p(-4.0, 6.0), p(4.0, 6.0)], stroke);
        }
        NavIcon::Settings => {
            for (dy, knob_x) in [(-5.0, 3.0), (0.0, -3.0), (5.0, 2.0)] {
                painter.line_segment([p(-7.0, dy), p(7.0, dy)], stroke);
                painter.circle_filled(p(knob_x, dy), 2.2, color);
            }
        }
        NavIcon::Output => {
            painter.line_segment([p(-7.0, -3.0), p(-3.0, -3.0)], stroke);
            painter.line_segment([p(-3.0, -3.0), p(-1.5, -5.0)], stroke);
            painter.line_segment([p(-1.5, -5.0), p(2.0, -5.0)], stroke);
            painter.line_segment([p(2.0, -5.0), p(3.0, -3.0)], stroke);
            painter.line_segment([p(3.0, -3.0), p(7.0, -3.0)], stroke);
            let r = egui::Rect::from_min_max(p(-7.0, -3.0), p(7.0, 6.0));
            painter.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
        }
        NavIcon::Create => {
            let r = egui::Rect::from_min_max(p(-5.5, -7.0), p(5.5, 7.0));
            painter.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
            painter.line_segment([p(-2.0, 0.0), p(2.0, 0.0)], stroke);
            painter.line_segment([p(0.0, -2.0), p(0.0, 2.0)], stroke);
        }
        NavIcon::Theme => {
            let radius = 6.5;
            let mut half = Vec::with_capacity(14);
            for step in 0..=12 {
                let angle =
                    std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (step as f32 / 12.0);
                half.push(center + Vec2::new(angle.cos() * radius, -angle.sin() * radius));
            }
            painter.add(egui::Shape::convex_polygon(half, color, Stroke::NONE));
            painter.circle_stroke(center, radius, stroke);
        }
    }
}

pub fn brand(ui: &mut Ui, language: Language) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(34.0, 24.0), Sense::hover());
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(4),
            token::surface(),
            Stroke::new(1.5_f32, token::ink()),
            StrokeKind::Inside,
        );
        painter.circle_filled(
            rect.right_top() + Vec2::new(-7.0, 7.0),
            2.6,
            token::record(),
        );
        painter.line_segment(
            [
                rect.left_bottom() + Vec2::new(6.0, -6.0),
                rect.right_bottom() + Vec2::new(-6.0, -6.0),
            ],
            Stroke::new(1.5_f32, token::ink()),
        );
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(
                RichText::new(i18n::t_main_title(language))
                    .size(14.5)
                    .color(token::ink())
                    .strong(),
            );
            ui.label(muted(
                match language {
                    Language::English => "Automatic slide capture",
                    Language::Chinese => "幻灯片自动截图",
                },
                10.5,
            ));
        });
    });
}

pub fn language_switch(ui: &mut Ui, active: Language) -> Option<Language> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for language in [Language::Chinese, Language::English] {
            let label = language.label();
            let selected = active == language;
            let button = Button::new(RichText::new(label).size(11.5).color(if selected {
                token::on_primary()
            } else {
                token::muted()
            }))
            .fill(if selected {
                token::primary()
            } else {
                token::surface()
            })
            .stroke(if selected {
                Stroke::NONE
            } else {
                Stroke::new(1.0_f32, token::line())
            })
            .corner_radius(6)
            .min_size(Vec2::new(48.0, 26.0));
            if ui.add(button).clicked() && !selected {
                chosen = Some(language);
            }
        }
    });
    chosen
}

/// A full-width row for window or display lists. Returns the interaction
/// response so callers can update their selection.
pub fn list_item(ui: &mut Ui, selected: bool, title: &str, detail: &str) -> Response {
    let height = if detail.is_empty() { 40.0 } else { 50.0 };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(8), token::primary_soft());
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(8), token::surface_soft());
    }

    let radio_center = egui::pos2(rect.left() + 16.0, rect.center().y);
    painter.circle_stroke(
        radio_center,
        6.0,
        Stroke::new(
            1.5_f32,
            if selected {
                token::accent()
            } else {
                token::line_strong()
            },
        ),
    );
    if selected {
        painter.circle_filled(radio_center, 2.8, token::accent());
    }

    let text_left = rect.left() + 32.0;
    let available = (rect.right() - text_left - 10.0).max(40.0);
    let title_galley = WidgetText::from(RichText::new(title).size(13.5).color(if selected {
        token::accent()
    } else {
        token::ink()
    }))
    .into_galley(ui, Some(TextWrapMode::Truncate), available, TextStyle::Body);
    if detail.is_empty() {
        let y = rect.center().y - title_galley.size().y / 2.0;
        painter.galley(egui::pos2(text_left, y), title_galley, token::ink());
    } else {
        painter.galley(
            egui::pos2(text_left, rect.center().y - 17.0),
            title_galley,
            token::ink(),
        );
        let detail_galley = WidgetText::from(
            RichText::new(detail)
                .monospace()
                .size(10.5)
                .color(token::muted()),
        )
        .into_galley(
            ui,
            Some(TextWrapMode::Truncate),
            available,
            TextStyle::Monospace,
        );
        painter.galley(
            egui::pos2(text_left, rect.center().y + 2.0),
            detail_galley,
            token::muted(),
        );
    }
    response
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    /// WCAG relative luminance, used only to prove the palettes stay readable.
    fn luminance(color: Color32) -> f32 {
        let channel = |value: u8| {
            let value = value as f32 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (first, second) = (luminance(a), luminance(b));
        let (high, low) = if first > second {
            (first, second)
        } else {
            (second, first)
        };
        (high + 0.05) / (low + 0.05)
    }

    fn assert_at_least(label: &str, ratio: f32, minimum: f32) {
        assert!(
            ratio >= minimum,
            "{label}: contrast {ratio:.2}:1 is below {minimum:.1}:1"
        );
    }

    /// Every role a panel actually combines. Text roles have to clear the WCAG AA
    /// body-text ratio (4.5:1), primary text the AAA ratio (7:1), and the faint
    /// metadata colour at least 3:1.
    fn check(label: &str, palette: Palette) {
        assert_at_least(
            &format!("{label}: ink on surface"),
            contrast(palette.ink, palette.surface),
            7.0,
        );
        assert_at_least(
            &format!("{label}: ink on page"),
            contrast(palette.ink, palette.bg),
            7.0,
        );
        assert_at_least(
            &format!("{label}: muted on surface"),
            contrast(palette.muted, palette.surface),
            4.5,
        );
        assert_at_least(
            &format!("{label}: muted on soft surface"),
            contrast(palette.muted, palette.surface_soft),
            4.5,
        );
        assert_at_least(
            &format!("{label}: faint on surface"),
            contrast(palette.faint, palette.surface),
            3.0,
        );
        assert_at_least(
            &format!("{label}: accent on surface"),
            contrast(palette.accent, palette.surface),
            4.5,
        );
        assert_at_least(
            &format!("{label}: accent on selected row"),
            contrast(palette.accent, palette.primary_soft),
            4.5,
        );
        assert_at_least(
            &format!("{label}: label on primary button"),
            contrast(palette.on_primary, palette.primary),
            4.5,
        );
        for (name, color) in [
            ("ok", palette.ok),
            ("warn", palette.warn),
            ("danger", palette.danger),
            ("record", palette.record),
            ("accent", palette.accent),
        ] {
            assert_at_least(
                &format!("{label}: {name} status text"),
                contrast(color, palette.surface),
                4.5,
            );
        }
    }

    #[test]
    fn the_light_palette_is_readable() {
        check("light", LIGHT);
    }

    #[test]
    fn the_dark_palette_is_readable() {
        check("dark", DARK);
    }

    #[test]
    fn the_two_palettes_are_actually_different() {
        assert_ne!(LIGHT.bg, DARK.bg);
        assert!(luminance(DARK.bg) < luminance(LIGHT.bg));
        assert!(luminance(DARK.ink) > luminance(DARK.surface));
        assert!(luminance(LIGHT.ink) < luminance(LIGHT.surface));
    }

    #[test]
    fn the_selected_row_lifts_off_the_sidebar_in_both_palettes() {
        // The active page has to be recognisable without relying on the accent
        // bar alone.
        assert!(contrast(LIGHT.nav_selected, LIGHT.sidebar) >= 1.1);
        assert!(contrast(DARK.nav_selected, DARK.sidebar) >= 1.1);
    }
}
