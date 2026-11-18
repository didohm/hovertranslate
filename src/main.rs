//! HoverTranslate - translate any text you rest the mouse pointer on.
//!
//! The app watches the pointer, reads the accessible text under it with the
//! Windows accessibility APIs, asks an online translation service and shows
//! the answer in a small card next to the pointer.

mod app;
mod bidi;
mod capture;
mod fonts;
mod hover;
mod languages;
mod settings;
mod theme;
mod translate;
mod ui;
mod uia;

use app::AppState;
use egui::{scroll_area::ScrollBarVisibility, ViewportId};
use settings::Settings;
use std::time::Duration;

const APP_TITLE: &str = "HoverTranslate";
const SCREENSHOT_FRAMES: u32 = 120;

fn main() {
    // The app has to know about the real pixels of the screen, otherwise the
    // pointer position and the accessibility APIs would not agree on a
    // scaled display.
    unsafe {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwareness, SetProcessDpiAwarenessContext,
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, PROCESS_DPI_AWARENESS,
        };
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
            let _ = SetProcessDpiAwareness(PROCESS_DPI_AWARENESS(0));
        }
    }

    let args: Vec<String> = std::env::args().collect();
    let screenshot = args
        .iter()
        .position(|arg| arg == "--screenshot")
        .and_then(|index| args.get(index + 1))
        .cloned();
    // A visual regression hook: show a long mixed Arabic paragraph in the
    // preview so the wrapping of right-to-left text can be checked in a picture.
    let rtl_demo = args.iter().any(|arg| arg == "--rtl-demo");

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("HoverTranslate");
        println!("  --screenshot <file.png>   draw a short while, save a picture and exit");
        println!("  --rtl-demo                show a long Arabic paragraph (for checking RTL)");
        return;
    }

    let settings = Settings::load();

    // The window starts at the size it will keep, so it does not have to
    // change its size while it is being drawn.
    let start_size = if settings.show_control_panel_on_start || screenshot.is_some() {
        egui::vec2(ui::PANEL_WIDTH, ui::PANEL_HEIGHT)
    } else {
        egui::vec2(ui::COMPACT_WIDTH, ui::COMPACT_HEIGHT)
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size(start_size)
            .with_min_inner_size([260.0, 48.0])
            .with_resizable(true),
        ..Default::default()
    };

    let result = eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(move |cc| {
            // The look of the app and the fonts it needs.
            theme::apply(&cc.egui_ctx);
            let report = fonts::install(&cc.egui_ctx);
            theme::set_variable_font(report.variable);
            println!(
                "[fonts] {}",
                if report.installed.is_empty() {
                    "no extra fonts found, the built-in ones are used".to_string()
                } else {
                    report.installed.join(", ")
                }
            );

            let mut state = AppState::new(settings);
            state.fonts = report;
            if screenshot.is_some() {
                // The picture always shows the whole panel in its normal,
                // running state, whatever the last session happened to save.
                state.compact = false;
                state.settings.enabled = true;
            }
            if rtl_demo {
                state.settings.target_language = "ar".to_string();
                state.popup = Some(demo_arabic_popup());
            }
            Ok(Box::new(HoverTranslateApp::new(state, screenshot)) as Box<dyn eframe::App>)
        }),
    );
    match result {
        Ok(()) => {}
        Err(error) => {
            eprintln!("HoverTranslate could not start: {error}");
            std::process::exit(1);
        }
    }
}

/// A long paragraph that mixes Arabic, English, digits and punctuation. It is
/// only used by `--rtl-demo` to check how right-to-left text wraps into lines.
fn demo_arabic_popup() -> app::Popup {
    let translation = concat!(
        "ضع المؤشر على أي نص لقراءته بلغتك، وانتظر 0.8 ثانية. تظهر الترجمة إلى ",
        "English في مربع صغير بجانب المؤشر بنسبة 50% عبر Example.com عند الساعة 9 صباحاً! ",
        "هذا نص طويلجداً ويجب أن يبدأ السطر الأول ببداية الجملة لا نهايتها 123.",
    )
    .to_string();
    app::Popup {
        source: "Rest the pointer on any text you want translated 123 and wait a moment."
            .to_string(),
        rect: (0, 0, 0, 0),
        pointer: (0, 0),
        translation: Some(translation),
        detected_source: Some("en".to_string()),
        error: None,
        error_kind: None,
        loading: false,
        from_cache: false,
        elapsed_ms: 12,
        provider: "demo".to_string(),
    }
}

struct HoverTranslateApp {
    state: AppState,
    screenshot: Option<String>,
    frames: u32,
    screenshot_requested: bool,
    finished: bool,
    popup_position: Option<egui::Pos2>,
    popup_size: Option<egui::Vec2>,
    window_size: Option<egui::Vec2>,
    last_compact: Option<bool>,
    last_save: std::time::Instant,
}

impl HoverTranslateApp {
    fn new(state: AppState, screenshot: Option<String>) -> Self {
        Self {
            state,
            screenshot,
            frames: 0,
            screenshot_requested: false,
            finished: false,
            popup_position: None,
            popup_size: None,
            window_size: None,
            last_compact: None,
            last_save: std::time::Instant::now(),
        }
    }

    fn draw_panel(&mut self, ui: &mut egui::Ui) {
        let mut content_height = 0.0f32;
        let footer_reserve = 40.0f32;

        let margin_frame = if self.state.compact {
            egui::Margin::same(8)
        } else {
            egui::Margin {
                left: 16,
                right: 16,
                top: 12,
                bottom: 10,
            }
        };
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::INK)
                    .inner_margin(margin_frame),
            )
            .show(ui, |ui| {
                if self.state.compact {
                    ui::draw_compact(ui, &mut self.state);
                } else {
                    let scroll_max_h = (ui.available_height() - footer_reserve).max(120.0);
                    let output = egui::ScrollArea::vertical()
                        .id_salt("control-panel-scroll")
                        .max_height(scroll_max_h)
                        .auto_shrink([false, true])
                        .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                        .show(ui, |ui| {
                            ui::draw_panel(ui, &mut self.state);
                        });
                    content_height = output.content_size.y;

                    ui.add_space(3.0);
                    theme::rule(ui);
                    ui.add_space(4.0);
                    ui::draw_panel_footer(ui);
                }
            });

        let pixels_per_point = ui.ctx().pixels_per_point();
        let (screen_width, screen_height) =
            match crate::hover::work_area(self.state.pointer.0, self.state.pointer.1) {
                Some((left, top, right, bottom)) => (
                    ((right - left).max(320) as f32) / pixels_per_point,
                    ((bottom - top).max(240) as f32) / pixels_per_point,
                ),
                None => {
                    let rect = ui.ctx().input(|i| i.viewport_rect());
                    (rect.width().max(480.0), rect.height().max(560.0))
                }
            };

        // Reserve 34pt for OS window titlebar and borders so the client area
        // never exceeds the monitor's usable work area.
        let max_client_height = (screen_height - 34.0).max(200.0);
        let max_client_width = (screen_width - 24.0).max(300.0);
        let wanted_height = (content_height + footer_reserve + 26.0)
            .min(max_client_height)
            .max(200.0);

        let is_compact = self.state.compact && self.screenshot.is_none();
        let mode_switched = self.last_compact != Some(is_compact);
        self.last_compact = Some(is_compact);

        // Preserve any user-chosen width when resizing the normal panel, while
        // clamping to the screen width.
        let current_width = ui
            .ctx()
            .input(|i| i.viewport().inner_rect.map(|r| r.width()))
            .unwrap_or(ui::PANEL_WIDTH);
        let target_width = if is_compact {
            ui::COMPACT_WIDTH.min(max_client_width)
        } else if mode_switched || self.screenshot.is_some() || current_width < 310.0 {
            ui::PANEL_WIDTH.min(max_client_width)
        } else {
            current_width.clamp(320.0, max_client_width)
        };

        let wanted = if is_compact {
            egui::vec2(target_width, ui::COMPACT_HEIGHT)
        } else {
            egui::vec2(target_width, wanted_height)
        };

        let height_changed = match self.window_size {
            Some(previous) => (previous.y - wanted.y).abs() > 3.0 || mode_switched,
            None => true,
        };
        if height_changed {
            self.window_size = Some(wanted);
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(wanted));
        }
    }

    /// The card next to the pointer lives in its own window.
    fn draw_popup(&mut self, ctx: &egui::Context) {
        let Some(popup) = self.state.popup.clone() else {
            self.state.popup_screen_rect = None;
            return;
        };
        let pixels_per_point = ctx.pixels_per_point();
        let target_language = self.state.settings.target_language.clone();

        let screen = match crate::hover::work_area(popup.pointer.0, popup.pointer.1) {
            Some((left, top, right, bottom)) => egui::Rect::from_min_max(
                egui::pos2(
                    left as f32 / pixels_per_point,
                    top as f32 / pixels_per_point,
                ),
                egui::pos2(
                    right as f32 / pixels_per_point,
                    bottom as f32 / pixels_per_point,
                ),
            ),
            None => ctx.input(|i| i.viewport_rect()),
        };

        // Responsive width and height based on content and screen size.
        let wanted_size = ui::popup_dimensions(
            ctx,
            &popup,
            self.state.settings.popup_max_width,
            screen.size(),
        );
        let width = wanted_size.x;
        let height = wanted_size.y;

        let pointer_x = popup.pointer.0 as f32 / pixels_per_point;
        let pointer_y = popup.pointer.1 as f32 / pixels_per_point;
        let mut x = pointer_x + self.state.settings.popup_offset_x;
        let mut y = pointer_y + self.state.settings.popup_offset_y;

        // Keep the card inside the usable screen area, flipping above or to the
        // left of the pointer when near screen edges so the hovered text stays visible.
        let edge_pad = 8.0;
        if x + width > screen.right() - edge_pad {
            let flipped_x = pointer_x - width - self.state.settings.popup_offset_x.max(12.0);
            x = if flipped_x >= screen.left() + edge_pad {
                flipped_x
            } else {
                (screen.right() - width - edge_pad).max(screen.left() + edge_pad)
            };
        }
        if y + height > screen.bottom() - edge_pad {
            let flipped_y = pointer_y - height - 14.0;
            y = if flipped_y >= screen.top() + edge_pad {
                flipped_y
            } else {
                (screen.bottom() - height - edge_pad).max(screen.top() + edge_pad)
            };
        }
        x = x.max(screen.left() + edge_pad);
        y = y.max(screen.top() + edge_pad);
        let position = egui::pos2(x, y);

        // Record the physical screen rectangle of the popup so the user can move
        // the pointer onto the popup to scroll long translations without closing it.
        self.state.popup_screen_rect = Some((
            (position.x * pixels_per_point).round() as i32,
            (position.y * pixels_per_point).round() as i32,
            ((position.x + width) * pixels_per_point).round() as i32,
            ((position.y + height) * pixels_per_point).round() as i32,
        ));

        // The window is only placed when it is created, so a popup that has to
        // follow the pointer is moved with a command.
        if let Some(last) = self.popup_position {
            if (last.x - position.x).abs() > 1.0 || (last.y - position.y).abs() > 1.0 {
                ctx.send_viewport_cmd_to(
                    ViewportId::from_hash_of("hovertranslate-popup"),
                    egui::ViewportCommand::OuterPosition(position),
                );
            }
        }
        self.popup_position = Some(position);

        if self.popup_size != Some(wanted_size) {
            self.popup_size = Some(wanted_size);
            ctx.send_viewport_cmd_to(
                ViewportId::from_hash_of("hovertranslate-popup"),
                egui::ViewportCommand::InnerSize(wanted_size),
            );
        }

        let builder = egui::ViewportBuilder::default()
            .with_title("HoverTranslate translation")
            .with_inner_size(wanted_size)
            .with_position(position)
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_transparent(true)
            .with_visible(true);

        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("hovertranslate-popup"),
            builder,
            |ui, _class| {
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui::draw_popup(ui, &popup, &target_language);
                    });
            },
        );
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        self.frames += 1;

        if self.frames == SCREENSHOT_FRAMES && !self.screenshot_requested {
            self.screenshot_requested = true;
            let context = ctx.clone();
            let target = path.clone();
            ctx.request_screenshot(move |image| {
                match capture::save_color_image(&image, &target) {
                    Ok(()) => println!("saved a picture to {target}"),
                    Err(error) => {
                        eprintln!("could not save the picture: {error}");
                        let fallback = capture::capture_window_to_png(APP_TITLE, &target);
                        match fallback {
                            Ok(()) => println!("saved a picture to {target}"),
                            Err(error) => eprintln!("no picture could be saved: {error}"),
                        }
                    }
                }
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            });
        }

        // If the graphics device never hands the pixels back, grab the window.
        if self.frames == SCREENSHOT_FRAMES + 240 {
            match capture::capture_window_to_png(APP_TITLE, &path) {
                Ok(()) => println!("saved a picture to {path}"),
                Err(error) => eprintln!("no picture could be saved: {error}"),
            }
            self.finished = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for HoverTranslateApp {
    /// Runs even when the window is hidden, so the app keeps working in the
    /// background.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.sync_settings();
        if self.screenshot.is_none() {
            self.state.poll_pointer();
        }
        self.state.poll_results();

        // Keep the settings safe even when the app runs for hours.
        if self.last_save.elapsed() > Duration::from_secs(20) {
            self.last_save = std::time::Instant::now();
            self.state.save_settings();
        }
        ctx.request_repaint_after(Duration::from_millis(80));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.screenshot.is_some() {
            // The picture shows the whole panel in its live state, never the
            // small bar or a paused switch left over from a previous session.
            self.state.compact = false;
            self.state.settings.enabled = true;
        }
        self.state.sync_settings();
        if self.screenshot.is_none() {
            self.state.poll_pointer();
        }
        self.state.poll_results();

        self.draw_panel(ui);
        self.draw_popup(&ctx);
        self.handle_screenshot(&ctx);
    }

    fn on_exit(&mut self) {
        self.state.save_settings();
    }
}
