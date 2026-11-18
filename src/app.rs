use crate::hover::{HoverEvent, HoverWatcher};
use crate::languages;
use crate::settings::{ProviderKind, Settings};
use crate::translate::{
    backend::BackendProvider, cache::TranslationCache, cache_key,
    libretranslate::LibreTranslateProvider, mymemory::MyMemoryProvider, ratelimit::RateLimiter,
    TranslateError, TranslationOutcome, TranslationProvider, TranslationRequest,
};
use crate::uia::GrabbedText;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// The little card that shows up next to the pointer.
#[derive(Debug, Clone)]
pub struct Popup {
    pub source: String,
    pub rect: (i32, i32, i32, i32),
    pub pointer: (i32, i32),
    pub translation: Option<String>,
    pub detected_source: Option<String>,
    pub error: Option<String>,
    pub error_kind: Option<String>,
    pub loading: bool,
    pub from_cache: bool,
    pub elapsed_ms: u128,
    pub provider: String,
}

impl Popup {
    fn new(source: String, rect: (i32, i32, i32, i32), pointer: (i32, i32)) -> Self {
        Self {
            source,
            rect,
            pointer,
            translation: None,
            detected_source: None,
            error: None,
            error_kind: None,
            loading: false,
            from_cache: false,
            elapsed_ms: 0,
            provider: String::new(),
        }
    }
}

struct TranslationResult {
    id: u64,
    source: String,
    outcome: Result<TranslationOutcome, TranslateError>,
}

/// Everything the app keeps in memory while it runs.
pub struct AppState {
    pub settings: Settings,
    pub watcher: HoverWatcher,
    pub provider: Arc<dyn TranslationProvider>,
    pub cache: TranslationCache,
    pub limiter: RateLimiter,

    pub popup: Option<Popup>,
    /// Screen-space bounding box of the rendered popup window so moving the
    /// pointer onto the popup to scroll long text does not dismiss it.
    pub popup_screen_rect: Option<(i32, i32, i32, i32)>,
    pending_id: Option<u64>,
    next_id: u64,

    results: Receiver<TranslationResult>,
    result_tx: Sender<TranslationResult>,

    pub status: String,
    pub last_action: Option<String>,
    pub sample_result: Option<(String, bool)>,
    pub sample_busy: bool,

    pub requests_made: u32,
    pub cache_hits: u32,
    pub compact: bool,
    pub pointer: (i32, i32),
    pub provider_error: Option<String>,
    provider_signature: String,
    pub remaining_requests: u64,
    pub accessibility_ready: bool,
    pub cache_size: usize,
    /// What the app found on this machine when it looked for fonts.
    pub fonts: crate::fonts::FontReport,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let watcher = HoverWatcher::start();
        watcher.set_delay(settings.hover_delay_secs);
        watcher.set_enabled(settings.enabled);

        let (result_tx, results) = channel();
        let provider = match build_provider(&settings) {
            Ok(provider) => provider,
            Err(_) => Arc::new(FailingProvider) as Arc<dyn TranslationProvider>,
        };
        let cache = TranslationCache::new(settings.cache_capacity, settings.cache_ttl_secs);
        let limiter = RateLimiter::new(settings.requests_per_minute);
        let signature = provider_signature(&settings);
        let start_compact = !settings.show_control_panel_on_start;
        let budget = settings.requests_per_minute as u64;

        Self {
            settings,
            watcher,
            provider,
            cache,
            limiter,
            popup: None,
            popup_screen_rect: None,
            pending_id: None,
            next_id: 1,
            results,
            result_tx,
            status: "Waiting for the pointer.".to_string(),
            last_action: None,
            sample_result: None,
            sample_busy: false,
            requests_made: 0,
            cache_hits: 0,
            compact: start_compact,
            pointer: (0, 0),
            provider_error: None,
            provider_signature: signature,
            remaining_requests: budget,
            accessibility_ready: true,
            cache_size: 0,
            fonts: crate::fonts::FontReport::default(),
        }
    }

    /// Push changed settings into the running parts of the app.
    pub fn sync_settings(&mut self) {
        self.watcher.set_delay(self.settings.hover_delay_secs);
        self.watcher.set_enabled(self.settings.enabled);
        self.limiter.resize(self.settings.requests_per_minute);
        self.remaining_requests = self.limiter.remaining() as u64;
        self.accessibility_ready = self.watcher.accessibility_ready();
        self.cache_size = self.cache.len();
        // "auto" in the source language means the service detects it.
        self.settings.detect_source_language = self.settings.source_language == languages::AUTO;

        let wanted = provider_signature(&self.settings);
        if wanted != self.provider_signature {
            match build_provider(&self.settings) {
                Ok(provider) => {
                    self.provider = provider;
                    self.provider_signature = wanted;
                    self.provider_error = None;
                }
                Err(message) => {
                    self.provider_error = Some(message);
                }
            }
        }
    }

    pub fn save_settings(&self) {
        let _ = self.settings.save();
    }

    /// Handle everything the pointer watcher noticed.
    pub fn poll_pointer(&mut self) {
        while let Some(event) = self.watcher.poll() {
            match event {
                HoverEvent::Moved { x, y } => {
                    self.pointer = (x, y);
                    let still_active = match &self.popup {
                        Some(popup) => {
                            let (left, top, right, bottom) = popup.rect;
                            let on_source = x >= left && x <= right && y >= top && y <= bottom;
                            let on_popup = self.is_pointer_on_popup(x, y);
                            on_source || on_popup
                        }
                        None => false,
                    };
                    if !still_active {
                        self.close_popup();
                    }
                }
                HoverEvent::Text(grabbed) => {
                    if self.is_pointer_on_popup(self.pointer.0, self.pointer.1) {
                        continue;
                    }
                    let text = grabbed.text.trim().to_string();
                    if text.is_empty() {
                        continue;
                    }
                    if let Some(popup) = &self.popup {
                        if popup.source == text {
                            continue;
                        }
                    }
                    self.start_translation(grabbed, text);
                }
                HoverEvent::NoText => {
                    if !self.is_pointer_on_popup(self.pointer.0, self.pointer.1) {
                        self.close_popup();
                    }
                }
            }
        }
    }

    fn is_pointer_on_popup(&self, x: i32, y: i32) -> bool {
        let Some((left, top, right, bottom)) = self.popup_screen_rect else {
            return false;
        };
        let margin = 20;
        x >= left - margin && x <= right + margin && y >= top - margin && y <= bottom + margin
    }

    fn close_popup(&mut self) {
        self.popup = None;
        self.popup_screen_rect = None;
        self.pending_id = None;
    }

    fn start_translation(&mut self, grabbed: GrabbedText, text: String) {
        let text: String = text.chars().take(self.settings.max_chars).collect();
        let request = TranslationRequest {
            text: text.clone(),
            // "auto" means the service works out the source language itself.
            source: self.settings.source_language.clone(),
            target: self.settings.target_language.clone(),
        };

        let rect = (
            grabbed.left,
            grabbed.top,
            grabbed.right,
            grabbed.bottom,
        );

        // 1. The cache answers straight away and costs nothing.
        let key = cache_key(self.provider.id(), &request);
        if let Some(hit) = self.cache.get(&key) {
            self.cache_hits += 1;
            let mut popup = Popup::new(text.clone(), rect, self.pointer);
            popup.translation = Some(hit);
            popup.from_cache = true;
            self.popup = Some(popup);
            self.status = "Answered from the cache.".to_string();
            return;
        }

        // 2. The rate limiter protects the free quota.
        if let Err(wait) = self.limiter.try_acquire() {
            let mut popup = Popup::new(text.clone(), rect, self.pointer);
            popup.error = Some(format!(
                "Too many requests. Try again in about {:.0} second(s).",
                wait.as_secs_f64().ceil()
            ));
            popup.error_kind = Some("rate limit".to_string());
            self.popup = Some(popup);
            self.status = "Rate limit reached.".to_string();
            return;
        }

        // 3. Ask the online service (on a worker thread).
        let id = self.next_id;
        self.next_id += 1;
        self.pending_id = Some(id);
        self.requests_made += 1;

        let provider = Arc::clone(&self.provider);
        let tx = self.result_tx.clone();
        let for_thread = text.clone();
        std::thread::spawn(move || {
            let result = provider.translate(&request);
            let _ = tx.send(TranslationResult {
                id,
                source: for_thread,
                outcome: result,
            });
        });

        let mut popup = Popup::new(text.clone(), rect, self.pointer);
        popup.loading = true;
        self.popup = Some(popup);
        self.status = "Asking the translation service...".to_string();
    }

    /// Pick up answers from the worker threads.
    pub fn poll_results(&mut self) {
        while let Ok(result) = self.results.try_recv() {
            if result.id == 0 {
                // The answer to the "test translation" button.
                self.sample_busy = false;
                self.sample_result = Some(match result.outcome {
                    Ok(outcome) => (outcome.translated, true),
                    Err(error) => (error.to_string(), false),
                });
                continue;
            }
            if self.pending_id != Some(result.id) {
                continue;
            }
            self.pending_id = None;

            let Some(popup) = self.popup.as_mut() else {
                continue;
            };
            if popup.source != result.source {
                continue;
            }

            match result.outcome {
                Ok(outcome) => {
                    println!(
                        "[translate] {:?} -> {:?} ({} ms, cache: {})",
                        result.source, outcome.translated, outcome.elapsed_ms, outcome.from_cache
                    );
                    popup.translation = Some(outcome.translated.clone());
                    popup.detected_source = outcome.detected_source.clone();
                    popup.loading = false;
                    popup.from_cache = outcome.from_cache;
                    popup.elapsed_ms = outcome.elapsed_ms;
                    popup.provider = outcome.provider.clone();

                    let request = TranslationRequest {
                        text: popup.source.clone(),
                        source: self.settings.source_language.clone(),
                        target: self.settings.target_language.clone(),
                    };
                    let key = cache_key(self.provider.id(), &request);
                    self.cache.put(key, outcome.translated.clone());

                    self.status = format!(
                        "Translated into {}. ({} ms)",
                        crate::settings::language_label(&self.settings.target_language),
                        outcome.elapsed_ms
                    );
                    self.last_action = Some(format!(
                        "{} → {}",
                        shorten(&popup.source, 40),
                        shorten(&outcome.translated, 40)
                    ));
                }
                Err(error) => {
                    println!("[translate] failed: {error}");
                    popup.loading = false;
                    popup.error = Some(error.to_string());
                    popup.error_kind = Some(error.kind().to_string());
                    self.status = match error {
                        TranslateError::Network(_) => {
                            "No internet connection - the translation needs one.".to_string()
                        }
                        TranslateError::RateLimited(_) => {
                            "The service is limiting requests, please try again.".to_string()
                        }
                        TranslateError::Rejected(_) => {
                            "The service refused the request.".to_string()
                        }
                        TranslateError::Unsupported(_) => {
                            "There was nothing to translate.".to_string()
                        }
                    };
                }
            }
        }
    }

    /// A quick check that the service answers, straight from the panel.
    pub fn test_translation(&mut self) {
        if self.sample_busy {
            return;
        }
        self.sample_busy = true;
        self.sample_result = None;

        let request = TranslationRequest {
            text: "Hello, how are you today?".to_string(),
            source: "auto".to_string(),
            target: self.settings.target_language.clone(),
        };
        let provider = Arc::clone(&self.provider);
        let tx = self.result_tx.clone();
        std::thread::spawn(move || {
            let result = provider.translate(&request);
            let _ = tx.send(TranslationResult {
                id: 0,
                source: "Hello, how are you today?".to_string(),
                outcome: result,
            });
        });
    }
}

fn provider_signature(settings: &Settings) -> String {
    match settings.provider {
        ProviderKind::FreePublic => "free".to_string(),
        ProviderKind::BackendRelay => format!("backend:{}", settings.backend_url.trim()),
        ProviderKind::LibreTranslate => format!("libre:{}", settings.libretranslate_url.trim()),
    }
}

/// Used when a service cannot be reached at all, so the app still starts.
struct FailingProvider;

impl TranslationProvider for FailingProvider {
    fn id(&self) -> &'static str {
        "unavailable"
    }

    fn label(&self) -> String {
        "No service available".to_string()
    }

    fn translate(
        &self,
        _request: &TranslationRequest,
    ) -> Result<TranslationOutcome, TranslateError> {
        Err(TranslateError::Rejected(
            "the translation service could not be prepared".to_string(),
        ))
    }
}

fn build_provider(settings: &Settings) -> Result<Arc<dyn TranslationProvider>, String> {    match settings.provider {
        ProviderKind::FreePublic => Ok(Arc::new(
            MyMemoryProvider::new().map_err(|e| e.to_string())?,
        )),
        ProviderKind::BackendRelay => Ok(Arc::new(
            BackendProvider::new(&settings.backend_url).map_err(|e| e.to_string())?,
        )),
        ProviderKind::LibreTranslate => Ok(Arc::new(
            LibreTranslateProvider::new(&settings.libretranslate_url).map_err(|e| e.to_string())?,
        )),
    }
}

pub fn shorten(text: &str, max: usize) -> String {
    let cleaned = text.replace('\n', " ").trim().to_string();
    if cleaned.chars().count() <= max {
        cleaned
    } else {
        let head: String = cleaned.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}
