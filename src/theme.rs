//! The look of the app: the colours, the sizes and the few widgets that egui
//! does not draw the way this app wants them.
//!
//! The app is a small instrument that lives on top of other programs, so the
//! look is quiet: a dark graphite ground, one accent colour, thin rules and
//! a lot of air. The only loud thing is the card that appears next to the
//! pointer.

use egui::{
    Color32, CornerRadius, Context, FontId, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, TextStyle, Ui, Vec2,
};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::fonts::WEIGHT_AXIS;

// The ground ---------------------------------------------------------------
pub const INK: Color32 = Color32::from_rgb(0x0E, 0x11, 0x16);
pub const SURFACE: Color32 = Color32::from_rgb(0x14, 0x19, 0x22);
pub const RAISED: Color32 = Color32::from_rgb(0x1A, 0x20, 0x2B);
pub const RAISED_HI: Color32 = Color32::from_rgb(0x23, 0x2A, 0x37);
pub const HAIRLINE: Color32 = Color32::from_rgb(0x23, 0x2A, 0x36);
pub const HAIRLINE_HI: Color32 = Color32::from_rgb(0x32, 0x3B, 0x4A);

// The ink ------------------------------------------------------------------
pub const TEXT: Color32 = Color32::from_rgb(0xE9, 0xEE, 0xF7);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x98, 0xA3, 0xB5);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6A, 0x75, 0x88);

// The one accent -----------------------------------------------------------
pub const IRIS: Color32 = Color32::from_rgb(0x7C, 0x8C, 0xFF);
pub const IRIS_DEEP: Color32 = Color32::from_rgb(0x4E, 0x5C, 0xC8);
pub const IRIS_WASH: Color32 = Color32::from_rgb(0x1B, 0x21, 0x3A);

// The states ---------------------------------------------------------------
pub const MINT: Color32 = Color32::from_rgb(0x4F, 0xD6, 0xA2);
pub const CORAL: Color32 = Color32::from_rgb(0xFF, 0x7B, 0x6B);
pub const AMBER: Color32 = Color32::from_rgb(0xF0, 0xB4, 0x5E);

// The sizes ----------------------------------------------------------------
pub const TITLE_SIZE: f32 = 21.0;
pub const BODY_SIZE: f32 = 13.5;
pub const ROW_SIZE: f32 = 14.0;
pub const SOURCE_SIZE: f32 = 13.0;
pub const META_SIZE: f32 = 12.0;
pub const TRANSLATION_SIZE: f32 = 18.0;
/// The text shown in the spacious translation preview card.
pub const PREVIEW_SIZE: f32 = 15.5;

/// True when the main font has a weight axis, so real weights can be used.
static VARIABLE: AtomicBool = AtomicBool::new(false);

pub fn set_variable_font(variable: bool) {
    VARIABLE.store(variable, Ordering::Relaxed);
}

pub fn has_variable_font() -> bool {
    VARIABLE.load(Ordering::Relaxed)
}

/// Put the house style into egui.
pub fn apply(ctx: &Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.global_style_mut(|style| {
    style.visuals.dark_mode = true;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = INK;
    style.visuals.window_fill = SURFACE;
    style.visuals.window_stroke = Stroke::new(1.0, HAIRLINE);
    style.visuals.window_corner_radius = CornerRadius::same(10);
    style.visuals.popup_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    style.visuals.extreme_bg_color = RAISED;
    style.visuals.faint_bg_color = SURFACE;
    style.visuals.text_edit_bg_color = Some(RAISED);
    style.visuals.code_bg_color = RAISED;
    style.visuals.warn_fg_color = AMBER;
    style.visuals.error_fg_color = CORAL;
    style.visuals.hyperlink_color = IRIS;
    style.visuals.selection.bg_fill = IRIS;
    style.visuals.selection.stroke = Stroke::new(1.0, IRIS);

    let widgets = &mut style.visuals.widgets;
    widgets.noninteractive.bg_fill = RAISED;
    widgets.noninteractive.weak_bg_fill = RAISED;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, HAIRLINE);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    widgets.inactive.bg_fill = RAISED;
    widgets.inactive.weak_bg_fill = RAISED;
    widgets.inactive.bg_stroke = Stroke::new(1.0, HAIRLINE_HI);
    widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    widgets.hovered.bg_fill = RAISED_HI;
    widgets.hovered.weak_bg_fill = RAISED_HI;
    widgets.hovered.bg_stroke = Stroke::new(1.0, IRIS_DEEP);
    widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    widgets.active.bg_fill = IRIS_DEEP;
    widgets.active.weak_bg_fill = IRIS_DEEP;
    widgets.active.bg_stroke = Stroke::new(1.0, IRIS);
    widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    widgets.open.bg_fill = RAISED_HI;
    widgets.open.weak_bg_fill = RAISED_HI;
    widgets.open.bg_stroke = Stroke::new(1.0, HAIRLINE_HI);
    widgets.open.fg_stroke = Stroke::new(1.0, TEXT);

    style.spacing.item_spacing = Vec2::new(12.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.slider_width = 180.0;

    style.text_styles.insert(TextStyle::Heading, FontId::new(TITLE_SIZE, egui::FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Body, FontId::new(BODY_SIZE, egui::FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Button, FontId::new(BODY_SIZE, egui::FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Small, FontId::new(META_SIZE, egui::FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Monospace, FontId::new(META_SIZE, egui::FontFamily::Monospace));

    });
}

// The type -----------------------------------------------------------------

fn styled(text: impl Into<String>, size: f32, weight: f32, color: Color32) -> RichText {
    let mut rich = RichText::new(text).size(size).color(color);
    if has_variable_font() {
        rich = rich.variation(WEIGHT_AXIS, weight);
    }
    rich
}

/// The name of the app, in the top corner.
pub fn title(text: impl Into<String>) -> RichText {
    styled(text, TITLE_SIZE, 620.0, TEXT)
}

/// The name of a group of settings.
pub fn section(text: impl Into<String>) -> RichText {
    styled(text, 12.0, 600.0, TEXT_FAINT)
}

/// A line of ordinary text.
pub fn body(text: impl Into<String>) -> RichText {
    styled(text, BODY_SIZE, 400.0, TEXT)
}

/// The name of a setting.
pub fn row_label(text: impl Into<String>) -> RichText {
    styled(text, ROW_SIZE, 400.0, TEXT)
}

/// The value of a setting.
pub fn value(text: impl Into<String>) -> RichText {
    styled(text, ROW_SIZE, 600.0, TEXT)
}

/// Small print under a setting.
pub fn meta(text: impl Into<String>) -> RichText {
    styled(text, META_SIZE, 400.0, TEXT_FAINT)
}

/// Source text shown in the translation popup card.
pub fn source_text(text: impl Into<String>) -> RichText {
    styled(text, SOURCE_SIZE, 400.0, TEXT_DIM)
}

/// The translation itself.
pub fn translation(text: impl Into<String>) -> RichText {
    styled(text, TRANSLATION_SIZE, 450.0, TEXT)
}

/// The text in the roomy preview card.
pub fn preview(text: impl Into<String>) -> RichText {
    styled(text, PREVIEW_SIZE, 430.0, TEXT)
}

/// Build an `egui::TextFormat` with the house font family and variable weight axis.
pub fn text_format(size: f32, weight: f32, color: Color32) -> egui::TextFormat {
    let mut coords = egui::epaint::text::VariationCoords::default();
    if has_variable_font() {
        coords.push(WEIGHT_AXIS, weight);
    }
    egui::TextFormat {
        font_id: FontId::new(size, egui::FontFamily::Proportional),
        color,
        coords,
        ..Default::default()
    }
}


// The widgets --------------------------------------------------------------

/// Mix two colours.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
        (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t) as u8,
    )
}

/// A hairline between two parts of the panel.
pub fn rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, HAIRLINE);
}

/// A small round light that shows a state.
pub fn lamp(ui: &mut Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), Sense::hover());
    let center = rect.center();
    let painter = ui.painter();
    painter.circle_filled(center, size * 0.5, color.gamma_multiply(0.22));
    painter.circle_filled(center, size * 0.28, color);
}

/// A switch that looks like the toggle of an instrument.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let size = Vec2::new(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        let id = ui.scope_id().with("switch");
        let anim = ui
            .ctx()
            .animate_bool_with_time(id, *on, 0.12)
            .clamp(0.0, 1.0);
        let painter = ui.painter();

        let track = Rect::from_center_size(rect.center(), size);
        let track_color = if anim > 0.0 {
            mix(IRIS_DEEP, IRIS, anim)
        } else {
            Color32::from_rgb(0x2A, 0x31, 0x3F)
        };
        painter.rect_filled(track, CornerRadius::same((size.y * 0.5) as u8), track_color);
        painter.rect_stroke(
            track,
            CornerRadius::same((size.y * 0.5) as u8),
            Stroke::new(
                1.0,
                if anim > 0.0 {
                    IRIS
                } else {
                    HAIRLINE_HI
                },
            ),
            StrokeKind::Middle,
        );

        // The knob slides from one side to the other.
        let travel = size.x - size.y;
        let x = track.left() + size.y * 0.5 + travel * anim;
        let knob = Rect::from_center_size(
            egui::pos2(x, track.center().y),
            Vec2::new(size.y - 5.0, size.y - 5.0),
        );
        painter.circle_filled(knob.center(), knob.width() * 0.5, Color32::WHITE);
        if anim <= 0.01 {
            painter.circle_stroke(
                knob.center(),
                knob.width() * 0.5,
                Stroke::new(1.0, HAIRLINE_HI),
            );
        }
    }
    response
}

/// The mark of the app: a lens with a light in it.
pub fn lens(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), Sense::hover());
    let painter = ui.painter();
    let radius = (size * 0.34) as u8;
    painter.rect_filled(rect, CornerRadius::same(radius), IRIS_WASH);
    painter.rect_stroke(
        rect,
        CornerRadius::same(radius),
        Stroke::new(1.0, Color32::from_rgb(0x36, 0x40, 0x6B)),
        StrokeKind::Middle,
    );

    let center = rect.center();
    let glass = size * 0.22;
    painter.circle_stroke(center, glass, Stroke::new(1.4, IRIS));
    painter.line_segment(
        [
            center + Vec2::new(-glass * 0.55, glass * 0.55),
            center + Vec2::new(glass * 0.05, -glass * 0.62),
        ],
        Stroke::new(1.4, Color32::from_rgb(0xC3, 0xCC, 0xFF)),
    );
}

/// A little button with a drawn picture instead of a word.
pub fn icon_button(ui: &mut Ui, icon: Icon) -> Response {
    let size = Vec2::new(26.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let painter = ui.painter();
        if hovered {
            painter.rect_filled(
                rect,
                CornerRadius::same(7),
                RAISED_HI,
            );
        }
        let color = if hovered { TEXT } else { TEXT_FAINT };
        icon.paint(painter, rect.center(), color);
    }
    response
}

/// The little pictures the app draws itself.
#[allow(dead_code)]
pub enum Icon {
    Gear,
    Cross,
    Arrow,
}

impl Icon {
    fn paint(&self, painter: &egui::Painter, center: egui::Pos2, color: Color32) {
        match self {
            Icon::Gear => {
                painter.circle_stroke(center, 5.5, Stroke::new(1.4, color));
                painter.circle_stroke(center, 2.0, Stroke::new(1.4, color));
                for index in 0..6 {
                    let angle = index as f32 * std::f32::consts::TAU / 6.0;
                    let direction = egui::vec2(angle.cos(), angle.sin());
                    painter.line_segment(
                        [
                            center + direction * 6.0,
                            center + direction * 8.0,
                        ],
                        Stroke::new(1.4, color),
                    );
                }
            }
            Icon::Cross => {
                painter.line_segment(
                    [
                        center + egui::vec2(-3.5, -3.5),
                        center + egui::vec2(3.5, 3.5),
                    ],
                    Stroke::new(1.4, color),
                );
                painter.line_segment(
                    [
                        center + egui::vec2(3.5, -3.5),
                        center + egui::vec2(-3.5, 3.5),
                    ],
                    Stroke::new(1.4, color),
                );
            }
            Icon::Arrow => {
                painter.line_segment(
                    [center + egui::vec2(-4.0, 0.0), center + egui::vec2(4.0, 0.0)],
                    Stroke::new(1.4, color),
                );
                painter.line_segment(
                    [
                        center + egui::vec2(0.5, -3.5),
                        center + egui::vec2(4.0, 0.0),
                    ],
                    Stroke::new(1.4, color),
                );
                painter.line_segment(
                    [
                        center + egui::vec2(0.5, 3.5),
                        center + egui::vec2(4.0, 0.0),
                    ],
                    Stroke::new(1.4, color),
                );
            }
        }
    }
}

