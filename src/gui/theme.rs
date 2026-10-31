//! Shared visual language: a light, quiet workspace with pine accents and a
//! single recording-red signal.

use egui::{
    self, Align2, Button, Color32, CornerRadius, CursorIcon, DragValue, FontFamily, FontId, Frame,
    Key, Margin, Response, RichText, Sense, Stroke, StrokeKind, TextStyle, TextWrapMode, Ui, Vec2,
    WidgetText,
};
use std::ops::RangeInclusive;

use crate::i18n::{self, Language};

pub mod token {
    use egui::Color32;

    pub const BG: Color32 = Color32::from_rgb(0xF3, 0xF6, 0xF4);
    pub const SIDEBAR: Color32 = Color32::from_rgb(0xEA, 0xEF, 0xEC);
    pub const SIDEBAR_HOVER: Color32 = Color32::from_rgb(0xE1, 0xE7, 0xE3);
    pub const SURFACE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const SURFACE_SOFT: Color32 = Color32::from_rgb(0xF1, 0xF4, 0xF2);
    pub const INK: Color32 = Color32::from_rgb(0x1C, 0x2C, 0x32);
    pub const MUTED: Color32 = Color32::from_rgb(0x64, 0x74, 0x79);
    pub const FAINT: Color32 = Color32::from_rgb(0x8B, 0x99, 0x9C);
    pub const LINE: Color32 = Color32::from_rgb(0xDC, 0xE3, 0xDE);
    pub const LINE_STRONG: Color32 = Color32::from_rgb(0xC3, 0xCE, 0xC8);
    pub const PRIMARY: Color32 = Color32::from_rgb(0x27, 0x4F, 0x46);
    pub const PRIMARY_SOFT: Color32 = Color32::from_rgb(0xE4, 0xEE, 0xE9);
    pub const RECORD: Color32 = Color32::from_rgb(0xBE, 0x47, 0x3C);
    pub const RECORD_SOFT: Color32 = Color32::from_rgb(0xFB, 0xF0, 0xED);
    pub const RECORD_LINE: Color32 = Color32::from_rgb(0xEC, 0xD0, 0xCA);
    pub const WARN: Color32 = Color32::from_rgb(0x8F, 0x65, 0x1E);
    pub const WARN_SOFT: Color32 = Color32::from_rgb(0xFA, 0xF3, 0xE4);
    pub const WARN_LINE: Color32 = Color32::from_rgb(0xE8, 0xD7, 0xB4);
    pub const DANGER: Color32 = Color32::from_rgb(0xA9, 0x39, 0x31);
    pub const DANGER_SOFT: Color32 = Color32::from_rgb(0xFA, 0xEE, 0xEC);
    pub const DANGER_LINE: Color32 = Color32::from_rgb(0xE9, 0xC9, 0xC5);
    pub const OK: Color32 = Color32::from_rgb(0x2E, 0x6B, 0x50);
    pub const OK_SOFT: Color32 = Color32::from_rgb(0xE8, 0xF2, 0xEC);
}

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
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
    .into();

    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = token::BG;
    visuals.window_fill = token::SURFACE;
    visuals.window_stroke = Stroke::new(1.0, token::LINE);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.extreme_bg_color = token::SURFACE_SOFT;
    visuals.faint_bg_color = token::SURFACE_SOFT;
    visuals.code_bg_color = token::SURFACE_SOFT;
    visuals.selection = egui::style::Selection {
        bg_fill: token::PRIMARY_SOFT,
        stroke: Stroke::new(1.0, token::PRIMARY),
    };
    visuals.hyperlink_color = token::PRIMARY;
    visuals.warn_fg_color = token::WARN;
    visuals.error_fg_color = token::DANGER;
    visuals.button_frame = true;
    visuals.striped = false;
    visuals.slider_trailing_fill = true;
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(36),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 22,
        spread: 0,
        color: Color32::from_black_alpha(32),
    };

    let radius = CornerRadius::same(4);
    visuals.widgets.noninteractive = egui::style::WidgetVisuals {
        bg_fill: token::SURFACE,
        weak_bg_fill: token::SURFACE,
        bg_stroke: Stroke::new(1.0, token::LINE),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, token::INK),
        expansion: 0.0,
    };
    visuals.widgets.inactive = egui::style::WidgetVisuals {
        bg_fill: token::SURFACE,
        weak_bg_fill: token::SURFACE_SOFT,
        bg_stroke: Stroke::new(1.0, token::LINE),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, token::INK),
        expansion: 0.0,
    };
    visuals.widgets.hovered = egui::style::WidgetVisuals {
        bg_fill: token::SURFACE,
        weak_bg_fill: token::SIDEBAR_HOVER,
        bg_stroke: Stroke::new(1.0, token::LINE_STRONG),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, token::INK),
        expansion: 0.0,
    };
    visuals.widgets.active = egui::style::WidgetVisuals {
        bg_fill: token::PRIMARY_SOFT,
        weak_bg_fill: token::PRIMARY_SOFT,
        bg_stroke: Stroke::new(1.0, token::PRIMARY),
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, token::PRIMARY),
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

    ctx.set_style(style);
}

pub fn card() -> Frame {
    Frame::new()
        .fill(token::SURFACE)
        .stroke(Stroke::new(1.0, token::LINE))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(20, 18))
}

pub fn banner_frame(fill: Color32, stroke: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(16, 12))
}

pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(26.0).color(token::INK).strong());
    ui.add_space(3.0);
    ui.label(RichText::new(subtitle).size(13.0).color(token::MUTED));
    ui.add_space(20.0);
}

pub fn section_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(token::MUTED).strong());
}

pub fn muted(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).size(size).color(token::MUTED)
}

pub fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .monospace()
        .size(11.0)
        .color(token::MUTED)
}

pub fn divider(ui: &mut Ui) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, token::LINE),
    );
    ui.add_space(10.0);
}

pub fn primary_button(text: impl Into<String>) -> Button<'static> {
    Button::new(
        RichText::new(text.into())
            .size(14.0)
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(token::PRIMARY)
    .stroke(Stroke::NONE)
    .corner_radius(8)
    .min_size(Vec2::new(0.0, 38.0))
}

pub fn secondary_button(text: impl Into<String>) -> Button<'static> {
    Button::new(RichText::new(text.into()).size(13.5).color(token::INK))
        .fill(token::SURFACE_SOFT)
        .stroke(Stroke::new(1.0, token::LINE))
        .corner_radius(8)
        .min_size(Vec2::new(0.0, 36.0))
}

pub fn danger_button(text: impl Into<String>) -> Button<'static> {
    Button::new(RichText::new(text.into()).size(13.5).color(token::DANGER))
        .fill(token::DANGER_SOFT)
        .stroke(Stroke::new(1.0, token::DANGER_LINE))
        .corner_radius(8)
        .min_size(Vec2::new(0.0, 36.0))
}

pub fn quiet_button(text: impl Into<String>) -> Button<'static> {
    Button::new(RichText::new(text.into()).size(12.0).color(token::PRIMARY)).frame(false)
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
            token::FAINT,
        );
        ui.label(RichText::new(text).size(12.5).color(token::MUTED));
    });
}

/// A soft tinted strip used for paths and short summaries.
pub fn soft_frame() -> Frame {
    Frame::new()
        .fill(token::SURFACE_SOFT)
        .stroke(Stroke::new(1.0, token::LINE))
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
    painter.rect_filled(rail, CornerRadius::same(3), token::LINE);
    painter.rect_filled(
        egui::Rect::from_min_max(rail.min, egui::pos2(center.x, rail.max.y)),
        CornerRadius::same(3),
        token::PRIMARY,
    );
    let active = response.hovered() || response.dragged() || response.has_focus();
    if response.has_focus() && !response.dragged() {
        painter.circle_stroke(center, 11.5, Stroke::new(1.0, token::PRIMARY_SOFT));
    }
    painter.circle(
        center,
        if active { 8.5 } else { 7.5 },
        token::SURFACE,
        Stroke::new(2.0, token::PRIMARY),
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
}

pub fn nav_item(ui: &mut Ui, selected: bool, icon: NavIcon, label: &str) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(9), token::SURFACE);
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.left_top() + Vec2::new(0.0, 10.0),
                Vec2::new(3.0, rect.height() - 20.0),
            ),
            CornerRadius::same(2),
            token::PRIMARY,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(9), token::SIDEBAR_HOVER);
    }

    let foreground = if selected {
        token::PRIMARY
    } else {
        token::MUTED
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
    let stroke = Stroke::new(1.5, color);
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
    }
}

pub fn brand(ui: &mut Ui, language: Language) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(34.0, 24.0), Sense::hover());
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(4),
            token::SURFACE,
            Stroke::new(1.5, token::INK),
            StrokeKind::Inside,
        );
        painter.circle_filled(rect.right_top() + Vec2::new(-7.0, 7.0), 2.6, token::RECORD);
        painter.line_segment(
            [
                rect.left_bottom() + Vec2::new(6.0, -6.0),
                rect.right_bottom() + Vec2::new(-6.0, -6.0),
            ],
            Stroke::new(1.5, token::INK),
        );
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(
                RichText::new(i18n::t_main_title(language))
                    .size(14.5)
                    .color(token::INK)
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
                Color32::WHITE
            } else {
                token::MUTED
            }))
            .fill(if selected {
                token::PRIMARY
            } else {
                token::SURFACE
            })
            .stroke(if selected {
                Stroke::NONE
            } else {
                Stroke::new(1.0, token::LINE)
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
        painter.rect_filled(rect, CornerRadius::same(8), token::PRIMARY_SOFT);
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(8), token::SURFACE_SOFT);
    }

    let radio_center = egui::pos2(rect.left() + 16.0, rect.center().y);
    painter.circle_stroke(
        radio_center,
        6.0,
        Stroke::new(
            1.5,
            if selected {
                token::PRIMARY
            } else {
                token::LINE_STRONG
            },
        ),
    );
    if selected {
        painter.circle_filled(radio_center, 2.8, token::PRIMARY);
    }

    let text_left = rect.left() + 32.0;
    let available = (rect.right() - text_left - 10.0).max(40.0);
    let title_galley = WidgetText::from(RichText::new(title).size(13.5).color(if selected {
        token::PRIMARY
    } else {
        token::INK
    }))
    .into_galley(ui, Some(TextWrapMode::Truncate), available, TextStyle::Body);
    if detail.is_empty() {
        let y = rect.center().y - title_galley.size().y / 2.0;
        painter.galley(egui::pos2(text_left, y), title_galley, token::INK);
    } else {
        painter.galley(
            egui::pos2(text_left, rect.center().y - 17.0),
            title_galley,
            token::INK,
        );
        let detail_galley = WidgetText::from(
            RichText::new(detail)
                .monospace()
                .size(10.5)
                .color(token::MUTED),
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
            token::MUTED,
        );
    }
    response
}
