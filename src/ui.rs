//! Drawing the app: the control panel and the card that appears next to the
//! pointer.
//!
//! The look is quiet on purpose. The panel is a small instrument that lives in
//! the background, so it is built from thin rules, one accent colour and a
//! lot of air. The card next to the pointer is the only thing that speaks up.

use crate::app::{AppState, Popup};
use crate::bidi;
use crate::languages;
use crate::settings::{sample_for, ProviderKind};
use crate::theme;
use egui::{
    scroll_area::ScrollBarVisibility, Align, CornerRadius, FontSelection, Layout, Margin, Rect,
    RichText, Sense, Stroke, TextEdit, TextFormat, Ui, Vec2, WidgetText,
};
use std::sync::Arc;

/// Default width and height of the control panel.
pub const PANEL_WIDTH: f32 = 480.0;
pub const PANEL_HEIGHT: f32 = 560.0;
pub const COMPACT_WIDTH: f32 = 264.0;
pub const COMPACT_HEIGHT: f32 = 48.0;

// ---------------------------------------------------------------------------
// Text in any script (LTR & RTL with UAX #9 per-line ordering)
// ---------------------------------------------------------------------------

/// Extract the `egui::TextFormat` produced by a `RichText` styling closure.
fn format_from_style(ui: &Ui, style: impl FnOnce(&str) -> RichText) -> TextFormat {
    let job = WidgetText::from(style(" ")).into_layout_job(
        ui.style(),
        FontSelection::Default,
        ui.text_valign(),
    );
    job.sections
        .first()
        .map(|s| s.format.clone())
        .unwrap_or_else(|| theme::text_format(theme::BODY_SIZE, 400.0, theme::TEXT))
}

/// Lay out a piece of text (Arabic, Hebrew, English, CJK, or mixed) into an
/// `egui::Galley` wrapped to `max_width`.
///
/// - For pure LTR text, delegates directly to `egui`'s standard single-section
///   `LayoutJob` so English layout is completely untouched.
/// - Whenever RTL characters are present:
///   1. Each `\n`-delimited paragraph is wrapped into lines in **logical**
///      order using the real font metrics so Line 1 always starts with the
///      beginning of the sentence and the last line ends with the end of the
///      sentence.
///   2. Each wrapped line's tokens are reordered into visual left-to-right
///      order per UAX #9 (`bidi::wrap_and_reorder`).
///   3. Each token is emitted as its own `LayoutSection` so HarfBuzz joins
///      Arabic letters within each word without ever merging or reversing
///      adjacent words, numbers, or punctuation.
pub fn build_bidi_galley(
    ctx: &egui::Context,
    text: &str,
    format: TextFormat,
    max_width: f32,
) -> Arc<egui::Galley> {
    let wrap_width = max_width.max(20.0);
    let is_rtl = bidi::base_is_rtl(text);
    let has_rtl = bidi::has_rtl(text);

    if !is_rtl && !has_rtl {
        let mut job = egui::text::LayoutJob::single_section(text.to_string(), format);
        job.wrap.max_width = wrap_width;
        job.break_on_newline = true;
        job.halign = Align::LEFT;
        return ctx.fonts_mut(|fonts| fonts.layout_job(job));
    }

    ctx.fonts_mut(|fonts| {
        let mut all_visual_lines: Vec<Vec<String>> = Vec::new();

        for paragraph in text.split('\n') {
            let para_rtl = if paragraph.trim().is_empty() {
                is_rtl
            } else {
                bidi::base_is_rtl(paragraph)
            };
            let lines = bidi::wrap_and_reorder(paragraph, para_rtl, wrap_width, |token_str| {
                if token_str.is_empty() {
                    return 0.0;
                }
                let tok_job = egui::text::LayoutJob {
                    sections: vec![egui::text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: egui::text::ByteIndex(0)
                            ..egui::text::ByteIndex(token_str.len()),
                        format: format.clone(),
                    }],
                    text: token_str.to_string(),
                    wrap: egui::text::TextWrapping {
                        max_width: f32::INFINITY,
                        ..Default::default()
                    },
                    break_on_newline: false,
                    ..Default::default()
                };
                fonts.layout_job(tok_job).size().x
            });
            all_visual_lines.extend(lines);
        }

        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = f32::INFINITY;
        job.break_on_newline = true;
        job.halign = if is_rtl { Align::RIGHT } else { Align::LEFT };

        for (line_idx, line_tokens) in all_visual_lines.into_iter().enumerate() {
            if line_idx > 0 {
                let start = job.text.len();
                job.text.push('\n');
                let end = job.text.len();
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: egui::text::ByteIndex(start)..egui::text::ByteIndex(end),
                    format: format.clone(),
                });
            }
            for tok in line_tokens {
                if tok.is_empty() {
                    continue;
                }
                let start = job.text.len();
                job.text.push_str(&tok);
                let end = job.text.len();
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: egui::text::ByteIndex(start)..egui::text::ByteIndex(end),
                    format: format.clone(),
                });
            }
        }

        if job.sections.is_empty() && !job.text.is_empty() {
            let end = job.text.len();
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: egui::text::ByteIndex(0)..egui::text::ByteIndex(end),
                format,
            });
        }

        fonts.layout_job(job)
    })
}

/// Draw a piece of text that may be Arabic, Hebrew, English, CJK or a mix.
///
/// Automatically detects paragraph direction (`bidi::base_is_rtl`), wraps
/// lines in logical order, orders bidirectional runs per line, and right-aligns
/// RTL blocks (`dir="rtl"`, `direction: rtl`, `text-align: right`) while
/// keeping LTR text left-aligned.
fn draw_bidi_text(ui: &mut Ui, text: &str, style: impl FnOnce(&str) -> RichText) {
    let format = format_from_style(ui, style);
    let wrap_width = ui.available_width().max(20.0);
    let is_rtl = bidi::base_is_rtl(text);
    let galley = build_bidi_galley(ui.ctx(), text, format, wrap_width);

    if is_rtl {
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            ui.add(egui::Label::new(galley));
        });
    } else {
        ui.add(egui::Label::new(galley));
    }
}

/// Measure the exact height of `text` when laid out at `width` with `format`.
fn bidi_text_height(ctx: &egui::Context, text: &str, format: TextFormat, width: f32) -> f32 {
    let min_h = format.font_id.size * 1.25;
    let galley = build_bidi_galley(ctx, text, format, width);
    galley.size().y.max(min_h)
}

/// Measure the natural (unwrapped) width of `text` on a single line.
fn bidi_single_line_width(ctx: &egui::Context, text: &str, format: TextFormat) -> f32 {
    let galley = build_bidi_galley(ctx, text, format, 2000.0);
    galley.size().x
}

// ---------------------------------------------------------------------------
// The control panel
// ---------------------------------------------------------------------------

pub fn draw_panel(ui: &mut egui::Ui, state: &mut AppState) {
    let available_w = ui.available_width();
    ui.set_max_width(available_w);
    ui.spacing_mut().item_spacing = Vec2::new(10.0, 4.0);

    header(ui, state);

    ui.add_space(4.0);
    theme::rule(ui);
    ui.add_space(5.0);

    ui.label(theme::section("Translation"));

    // The switch -------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label(theme::row_label("Translate on hover"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            theme::switch(ui, &mut state.settings.enabled);
        });
    });

    ui.add_space(1.0);
    theme::rule(ui);
    ui.add_space(4.0);

    // The delay --------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label(theme::row_label("Hover delay"));
        ui.add_space(4.0);
        slider(ui, &mut state.settings.hover_delay_secs, 0.2..=3.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::value(format!("{:.1} s", state.settings.hover_delay_secs)));
        });
    });

    ui.add_space(1.0);
    theme::rule(ui);
    ui.add_space(4.0);

    let combo_width = (available_w * 0.52).clamp(148.0, 250.0);

    // The languages ----------------------------------------------------
    {
        let selected = languages::language_label(&state.settings.target_language);
        ui.horizontal(|ui| {
            ui.label(theme::row_label("Translate into"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::ComboBox::from_id_salt("target-language")
                    .selected_text(selected)
                    .width(combo_width)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for language in languages::LANGUAGES {
                            ui.selectable_value(
                                &mut state.settings.target_language,
                                language.code.to_string(),
                                language.name,
                            );
                        }
                    });
            });
        });
    }

    // A roomy, responsive card that shows how the answer reads, and the last
    // real translation when there is one. It grows with its text and scrolls
    // when the text is long, so nothing is ever cut off.
    ui.add_space(3.0);
    draw_preview(ui, state);
    ui.add_space(3.0);

    {
        ui.add_space(2.0);
        let selected = match state.settings.source_language.as_str() {
            languages::AUTO => "Detect automatically".to_string(),
            other => languages::language_label(other),
        };
        ui.horizontal(|ui| {
            ui.label(theme::row_label("Source language"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::ComboBox::from_id_salt("source-language")
                    .selected_text(selected)
                    .width(combo_width)
                    .truncate()
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.settings.source_language,
                            languages::AUTO.to_string(),
                            "Detect automatically",
                        );
                        for language in languages::LANGUAGES {
                            ui.selectable_value(
                                &mut state.settings.source_language,
                                language.code.to_string(),
                                language.name,
                            );
                        }
                    });
            });
        });
    }

    ui.add_space(4.0);
    theme::rule(ui);
    ui.add_space(4.0);

    // The service ------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label(theme::row_label("Translation service"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            egui::ComboBox::from_id_salt("provider")
                .selected_text(state.settings.provider.label())
                .width(combo_width)
                .truncate()
                .show_ui(ui, |ui| {
                    for kind in ProviderKind::all() {
                        ui.selectable_value(&mut state.settings.provider, kind, kind.label());
                    }
                });
        });
    });

    match state.settings.provider {
        ProviderKind::BackendRelay => {
            ui.add(
                TextEdit::singleline(&mut state.settings.backend_url)
                    .hint_text("https://my-server.example.com/translate")
                    .desired_width(f32::INFINITY),
            );
        }
        ProviderKind::LibreTranslate => {
            ui.add(
                TextEdit::singleline(&mut state.settings.libretranslate_url)
                    .hint_text("https://libretranslate.example.com/translate")
                    .desired_width(f32::INFINITY),
            );
            if !languages::provider_supports(
                ProviderKind::LibreTranslate,
                &state.settings.target_language,
            ) {
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "This instance knows one kind of Chinese; traditional characters \
                             may come back in the simplified form.",
                        )
                        .color(theme::AMBER)
                        .size(theme::META_SIZE),
                    )
                    .wrap(),
                );
            }
        }
        ProviderKind::FreePublic => {}
    }

    if let Some(error) = &state.provider_error {
        egui::ScrollArea::vertical()
            .id_salt("provider-error-scroll")
            .max_height(54.0)
            .auto_shrink([false, true])
            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                draw_bidi_text(ui, error, |s| {
                    RichText::new(s).color(theme::CORAL).size(theme::META_SIZE)
                });
            });
    }

    ui.add_space(3.0);
    theme::rule(ui);
    ui.add_space(4.0);

    // The state --------------------------------------------------------
    status_line(ui, state);

    ui.add(
        egui::Label::new(theme::meta(format!(
            "{} {}  ·  {} cached  ·  {} left this minute",
            state.requests_made,
            if state.requests_made == 1 {
                "request"
            } else {
                "requests"
            },
            state.cache_hits,
            state.remaining_requests
        )))
        .wrap(),
    );
    ui.horizontal_wrapped(|ui| {
        if ui.add(egui::Button::new(theme::body("Compact mode"))).clicked() {
            state.compact = true;
        }
        if primary_button(ui, "Test the service").clicked() {
            state.test_translation();
        }
    });

    if let Some((text, ok)) = &state.sample_result {
        let color = if *ok { theme::MINT } else { theme::CORAL };
        egui::ScrollArea::vertical()
            .id_salt("sample-result-scroll")
            .max_height(56.0)
            .auto_shrink([false, true])
            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                draw_bidi_text(ui, text, |s| {
                    RichText::new(s).color(color).size(theme::META_SIZE)
                });
            });
    }
}

/// The footer of the control panel, rendered in a pinned bottom panel so the
/// privacy notice and Quit button are never cut off on any screen size.
pub fn draw_panel_footer(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let right_reserve = 74.0;
        let label_width = (ui.available_width() - right_reserve).max(120.0);
        ui.allocate_ui_with_layout(
            Vec2::new(label_width, 28.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(theme::meta(
                        "Only the hovered text is sent; nothing is written to disk.",
                    ))
                    .wrap(),
                );
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(egui::Button::new(theme::body("Quit"))).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    });
}

/// The roomy card that previews a translation. It shows the last real result
/// when there is one, otherwise a short sentence in the chosen language.
/// The card grows with its text and scrolls when the text is long, so no word
/// and no button is ever cut off.
fn draw_preview(ui: &mut Ui, state: &AppState) {
    let (text, sample) = match &state.popup {
        Some(popup) => match (&popup.translation, &popup.error) {
            (Some(translation), _) => (translation.clone(), false),
            (None, Some(error)) => (error.clone(), false),
            _ => (sample_for(&state.settings.target_language), true),
        },
        None => match &state.sample_result {
            Some((translated, true)) if !translated.trim().is_empty() => {
                (translated.clone(), false)
            }
            _ => (sample_for(&state.settings.target_language), true),
        },
    };

    ui.label(theme::section(if sample {
        "TRANSLATION PREVIEW"
    } else {
        "LATEST TRANSLATION"
    }));
    ui.add_space(2.0);

    let inner_width = (ui.available_width() - 32.0).max(120.0);
    let preview_format = theme::text_format(theme::PREVIEW_SIZE, 430.0, theme::TEXT);
    let measured = bidi_text_height(ui.ctx(), &text, preview_format, inner_width);
    // Grow flexibly with the real measured text height so 1-to-4-line paragraphs
    // fit without cutting off a single line, and scroll smoothly for longer texts.
    let wanted = (measured + 4.0).clamp(34.0, 108.0);

    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::HAIRLINE_HI))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("preview-card-scroll")
                .max_height(wanted)
                .auto_shrink([false, true])
                .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    draw_bidi_text(ui, &text, |s| theme::preview(s));
                });
        });
}

fn header(ui: &mut Ui, state: &AppState) {
    ui.horizontal(|ui| {
        theme::lens(ui, 32.0);
        ui.add_space(2.0);
        let text_col_w = (ui.available_width() - 72.0).max(120.0);
        ui.allocate_ui_with_layout(
            Vec2::new(text_col_w, 38.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.label(theme::title("HoverTranslate"));
                ui.add(
                    egui::Label::new(theme::meta(
                        "Rest the pointer on any text to read it in your language",
                    ))
                    .wrap(),
                );
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (text, color) = if state.settings.enabled {
                ("Live", theme::MINT)
            } else {
                ("Paused", theme::TEXT_FAINT)
            };
            ui.label(theme::meta(text).color(color));
            theme::lamp(ui, color, 9.0);
        });
    });
}

/// The line that tells how the app is doing.
fn status_line(ui: &mut Ui, state: &AppState) {
    ui.horizontal(|ui| {
        let (color, text) = if !state.settings.enabled {
            (theme::TEXT_FAINT, "Paused".to_string())
        } else if state.sample_busy {
            (theme::AMBER, "Asking the service...".to_string())
        } else {
            (theme::MINT, state.status.clone())
        };
        theme::lamp(ui, color, 8.0);
        ui.add_space(2.0);
        draw_bidi_text(ui, &text, |s| theme::meta(s));
    });

    if !state.accessibility_ready {
        ui.add(
            egui::Label::new(
                RichText::new(
                    "Windows accessibility could not be reached, so no text can be read yet.",
                )
                .color(theme::AMBER)
                .size(theme::META_SIZE),
            )
            .wrap(),
        );
    }
}

/// A slider that looks like the scale of an instrument.
fn slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    let height = 20.0;
    let width = (ui.available_width() - 56.0).clamp(72.0, 200.0);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

    let (min, max) = (*range.start(), *range.end());
    let span = (max - min).max(0.0001);
    let mut fraction = ((*value - min) / span).clamp(0.0, 1.0);

    if response.dragged() || response.clicked() {
        if let Some(pointer) = response.interact_pointer_pos() {
            fraction = ((pointer.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
            *value = (min + span * fraction * 100.0).round() / 100.0;
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let y = rect.center().y;
        let track = Rect::from_min_max(
            egui::pos2(rect.left(), y - 1.0),
            egui::pos2(rect.right(), y + 1.0),
        );
        painter.rect_filled(track, 1.0, theme::HAIRLINE_HI);
        let filled = Rect::from_min_max(
            egui::pos2(rect.left(), y - 1.0),
            egui::pos2(rect.left() + rect.width() * fraction, y + 1.0),
        );
        painter.rect_filled(filled, 1.0, theme::IRIS_DEEP);

        let knob_x = rect.left() + rect.width() * fraction;
        let knob = egui::pos2(knob_x, y);
        painter.circle_filled(knob, 7.0, theme::RAISED_HI);
        painter.circle_stroke(knob, 7.0, Stroke::new(2.0, theme::IRIS));
        painter.circle_filled(knob, 2.0, theme::IRIS);
    }
}

/// A button that stands out a little.
fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.scope(|ui| {
        let style = ui.style_mut();
        style.visuals.widgets.inactive.bg_fill = theme::IRIS_DEEP;
        style.visuals.widgets.inactive.weak_bg_fill = theme::IRIS_DEEP;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme::IRIS);
        style.visuals.widgets.hovered.bg_fill = theme::IRIS;
        style.visuals.widgets.hovered.weak_bg_fill = theme::IRIS;
        style.visuals.widgets.active.bg_fill = theme::IRIS;
        style.visuals.widgets.active.weak_bg_fill = theme::IRIS;
        ui.add(egui::Button::new(theme::body(text)))
    })
    .inner
}

// ---------------------------------------------------------------------------
// The card next to the pointer
// ---------------------------------------------------------------------------

/// The card that is drawn next to the pointer.
pub fn draw_popup(ui: &mut egui::Ui, popup: &Popup, target_language: &str) {
    let available = ui.available_size();
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::HAIRLINE_HI))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            let inner_w = (available.x - 30.0).max(140.0);
            let inner_h = (available.y - 30.0).max(90.0);
            ui.set_min_width(inner_w);
            ui.set_max_width(inner_w);
            draw_card_responsive(ui, popup, target_language, inner_h);
        });
}

/// The content of the card, with a pinned header and footer and a scrollable
/// body so long source or translated paragraphs scroll cleanly without ever
/// cutting off the header, footer, or card border.
#[allow(dead_code)]
pub fn draw_card(ui: &mut Ui, popup: &Popup, target_language: &str) {
    let inner_h = ui.available_height().max(180.0);
    draw_card_responsive(ui, popup, target_language, inner_h);
}

fn draw_card_responsive(
    ui: &mut Ui,
    popup: &Popup,
    target_language: &str,
    available_inner_height: f32,
) {
    let width = ui.available_width();
    ui.set_max_width(width);
    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);

    // 1. Pinned Header: language pair and status indicator.
    ui.horizontal(|ui| {
        let source = popup
            .detected_source
            .clone()
            .unwrap_or_else(|| "auto".to_string());
        ui.label(theme::meta(format!(
            "{} → {}",
            source.to_uppercase(),
            target_language.to_uppercase()
        )));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let color = if popup.loading {
                theme::AMBER
            } else if popup.error.is_some() {
                theme::CORAL
            } else {
                theme::MINT
            };
            theme::lamp(ui, color, 7.0);
            ui.add_space(4.0);
            let note = if popup.loading {
                "working"
            } else if popup.error.is_some() {
                "problem"
            } else {
                "ready"
            };
            ui.label(theme::meta(note));
        });
    });

    ui.add_space(7.0);
    theme::rule(ui);
    ui.add_space(8.0);

    // Reserve vertical space for the header (~32pt) and footer (~32pt) so the
    // scrollable body fits inside the card without pushing the footer off-screen.
    let body_max_height = (available_inner_height - 64.0).max(56.0);

    // 2. Scrollable Body: picked-up source text + translated text.
    egui::ScrollArea::vertical()
        .id_salt("popup-card-body-scroll")
        .max_height(body_max_height)
        .auto_shrink([false, true])
        .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            draw_bidi_text(ui, &popup.source, |s| theme::source_text(s));
            ui.add_space(8.0);

            if popup.loading {
                ui.add(egui::Label::new(theme::translation("Translating…")).wrap());
            } else if let Some(translation) = &popup.translation {
                draw_bidi_text(ui, translation, |s| theme::translation(s));
            } else if let Some(error) = &popup.error {
                draw_bidi_text(ui, error, |s| {
                    RichText::new(s).color(theme::CORAL).size(theme::BODY_SIZE + 0.5)
                });
            }
        });

    ui.add_space(9.0);
    theme::rule(ui);
    ui.add_space(6.0);

    // 3. Pinned Footer: service and latency metadata.
    let mut parts: Vec<String> = Vec::new();
    if popup.provider.is_empty() {
        parts.push("free service".to_string());
    } else {
        parts.push(popup.provider.clone());
    }
    if popup.from_cache {
        parts.push("from the cache".to_string());
    } else if popup.elapsed_ms > 0 {
        parts.push(format!("{} ms", popup.elapsed_ms));
    }
    ui.add(egui::Label::new(theme::meta(parts.join("  ·  "))).truncate());
}

/// Compute responsive `(width, height)` for the floating translation popup
/// based on the content length and the available screen dimensions.
pub fn popup_dimensions(
    ctx: &egui::Context,
    popup: &Popup,
    configured_max_width: f32,
    screen_size: Vec2,
) -> Vec2 {
    let max_screen_w = (screen_size.x - 24.0).max(240.0);
    let max_screen_h = (screen_size.y - 24.0).max(180.0);

    let source = popup.source.as_str();
    let translation = popup
        .translation
        .as_deref()
        .or_else(|| popup.error.as_deref())
        .unwrap_or("Translating…");

    let total_chars = source.chars().count() + translation.chars().count();

    // Adapt maximum width to content length and screen size:
    // - Short/medium content respects configured_max_width (~420pt).
    // - Long paragraphs widen up to 540pt (or 55% of screen width) for comfortable reading.
    let adaptive_max_w = if total_chars > 240 {
        configured_max_width
            .max(520.0)
            .min((screen_size.x * 0.55).max(380.0))
    } else if total_chars > 120 {
        configured_max_width
            .max(450.0)
            .min((screen_size.x * 0.50).max(360.0))
    } else {
        configured_max_width
    }
    .min(max_screen_w);

    let min_w = 280.0_f32.min(adaptive_max_w);

    let source_fmt = theme::text_format(theme::SOURCE_SIZE, 400.0, theme::TEXT_DIM);
    let trans_fmt = theme::text_format(theme::TRANSLATION_SIZE, 450.0, theme::TEXT);

    let natural_source_w = bidi_single_line_width(ctx, source, source_fmt);
    let natural_trans_w = bidi_single_line_width(ctx, translation, trans_fmt);
    let natural_card_w = natural_source_w.max(natural_trans_w) + 56.0;

    let width = natural_card_w.clamp(min_w, adaptive_max_w);
    let height = popup_height_for_screen(ctx, popup, width, max_screen_h);

    Vec2::new(width, height)
}

/// How tall the card will be for a given `width`, clamped to the screen's usable height.
pub fn popup_height_for_screen(
    ctx: &egui::Context,
    popup: &Popup,
    width: f32,
    max_screen_h: f32,
) -> f32 {
    // Subtract outer viewport margin (12pt), frame inner margin (28pt), frame stroke (2pt),
    // and scrollbar reserve (14pt) so our measurement matches the exact wrap width inside ScrollArea.
    let inner = (width - 56.0).max(120.0);
    let source = popup.source.as_str();
    let translation = popup
        .translation
        .as_deref()
        .or_else(|| popup.error.as_deref())
        .unwrap_or("Translating…");

    let source_fmt = theme::text_format(theme::SOURCE_SIZE, 400.0, theme::TEXT_DIM);
    let trans_fmt = theme::text_format(theme::TRANSLATION_SIZE, 450.0, theme::TEXT);

    let source_height = bidi_text_height(ctx, source, source_fmt, inner);
    let translation_height = bidi_text_height(ctx, translation, trans_fmt, inner);

    // Chrome: outer margin (12) + frame margin/stroke (30) + header (32) + body gap (8) + footer (32) + breathing room (8)
    let measured = 12.0 + 30.0 + 32.0 + source_height + 8.0 + translation_height + 32.0 + 8.0;
    let max_h = (max_screen_h * 0.72).clamp(240.0, 520.0).min(max_screen_h);
    measured.clamp(146.0, max_h)
}

/// How tall the card will be, so the window around it can be the right size.
#[allow(dead_code)]
pub fn popup_height(ctx: &egui::Context, popup: &Popup, width: f32) -> f32 {
    popup_height_for_screen(ctx, popup, width, 720.0)
}

/// The small bar that stays on screen when the app runs in the background.
pub fn draw_compact(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal_centered(|ui| {
        let color = if state.settings.enabled {
            theme::MINT
        } else {
            theme::TEXT_FAINT
        };
        theme::lamp(ui, color, 9.0);
        ui.add_space(4.0);
        let label_max_w = (ui.available_width() - 68.0).max(60.0);
        ui.allocate_ui_with_layout(
            Vec2::new(label_max_w, 26.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(theme::body(languages::language_label(
                        &state.settings.target_language,
                    )))
                    .truncate(),
                );
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::icon_button(ui, theme::Icon::Cross).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if theme::icon_button(ui, theme::Icon::Gear).clicked() {
                state.compact = false;
            }
        });
    });
}
