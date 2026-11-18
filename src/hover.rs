//! Watches the mouse pointer and reports the text that sits under it.
//!
//! The watcher runs on its own thread so a slow accessibility call can never
//! freeze the user interface.

use crate::uia::{GrabbedText, TextGrabber};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// What the watcher tells the app about.
#[derive(Debug, Clone)]
pub enum HoverEvent {
    /// The pointer moved to a new place.
    Moved { x: i32, y: i32 },
    /// Accessible text was found under the (still) pointer.
    Text(GrabbedText),
    /// The pointer is over something that does not expose any text.
    NoText,
}

const POLL_INTERVAL: Duration = Duration::from_millis(40);

pub struct HoverWatcher {
    events: Receiver<HoverEvent>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    delay_ms: Arc<AtomicU64>,
    enabled: Arc<AtomicBool>,
    accessibility: Arc<AtomicBool>,
}

impl HoverWatcher {
    pub fn start() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let delay_ms = Arc::new(AtomicU64::new(800));
        let enabled = Arc::new(AtomicBool::new(true));
        let accessibility = Arc::new(AtomicBool::new(false));

        let thread_stop = Arc::clone(&stop);
        let thread_delay = Arc::clone(&delay_ms);
        let thread_enabled = Arc::clone(&enabled);
        let thread_accessibility = Arc::clone(&accessibility);
        let handle = std::thread::Builder::new()
            .name("hover-watcher".to_string())
            .spawn(move || {
                run(
                    tx,
                    thread_stop,
                    thread_delay,
                    thread_enabled,
                    thread_accessibility,
                );
            })
            .ok();

        Self {
            events: rx,
            stop,
            handle,
            delay_ms,
            enabled,
            accessibility,
        }
    }

    /// True when the Windows accessibility APIs could be reached.
    pub fn accessibility_ready(&self) -> bool {
        self.accessibility.load(Ordering::Relaxed)
    }

    /// Change the hover delay without restarting the thread.
    pub fn set_delay(&self, seconds: f32) {
        self.delay_ms
            .store((seconds.max(0.05) * 1000.0) as u64, Ordering::Relaxed);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Take the next event, if there is one.
    pub fn poll(&self) -> Option<HoverEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for HoverWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run(
    tx: Sender<HoverEvent>,
    stop: Arc<AtomicBool>,
    delay_ms: Arc<AtomicU64>,
    enabled: Arc<AtomicBool>,
    accessibility: Arc<AtomicBool>,
) {
    let grabber = TextGrabber::new();
    accessibility.store(grabber.is_available(), Ordering::Relaxed);
    let mut last_position: Option<(i32, i32)> = None;
    let mut hovering_since: Option<Instant> = None;
    let mut inspected_position: Option<(i32, i32)> = None;

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(POLL_INTERVAL);
        if !enabled.load(Ordering::Relaxed) {
            last_position = None;
            hovering_since = None;
            inspected_position = None;
            continue;
        }

        let Some((x, y)) = cursor_position() else {
            continue;
        };

        let moved = last_position != Some((x, y));
        if moved {
            last_position = Some((x, y));
            hovering_since = Some(Instant::now());
            inspected_position = None;
            if tx.send(HoverEvent::Moved { x, y }).is_err() {
                break;
            }
            continue;
        }

        let since = hovering_since.unwrap_or_else(Instant::now);
        let delay = Duration::from_millis(delay_ms.load(Ordering::Relaxed));
        if since.elapsed() < delay {
            continue;
        }
        if inspected_position == Some((x, y)) {
            continue;
        }
        inspected_position = Some((x, y));

        let event = match grabber.grab(x, y) {
            Some(grabbed) => {
                println!(
                    "[hover] found text at ({x}, {y}): {:?}",
                    grabbed.text.chars().take(60).collect::<String>()
                );
                HoverEvent::Text(grabbed)
            }
            None => {
                println!("[hover] no accessible text at ({x}, {y})");
                HoverEvent::NoText
            }
        };
        if tx.send(event).is_err() {
            break;
        }
    }
}

/// Where is the pointer right now, in screen pixels?
pub fn cursor_position() -> Option<(i32, i32)> {
    unsafe {
        let mut point = windows::Win32::Foundation::POINT::default();
        if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point).is_ok() {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}

/// The usable area of the screen the pointer is on, in screen pixels.
pub fn work_area(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    unsafe {
        let monitor = MonitorFromPoint(
            POINT { x, y },
            MONITOR_DEFAULTTONEAREST,
        );
        if monitor.0.is_null() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let area = info.rcWork;
            return Some((area.left, area.top, area.right, area.bottom));
        }
        None
    }
}
