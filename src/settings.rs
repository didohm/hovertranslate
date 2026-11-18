use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// The list of languages lives in its own module, because it also knows how
// every service names them.
pub use crate::languages::{language_label, sample_for};

/// Which online service the app talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    /// Free, key-less public endpoint (MyMemory). Handy for the first release.
    FreePublic,
    /// A self-hosted / team backend that owns the provider API key.
    BackendRelay,
    /// A LibreTranslate instance (self-hosted or a provider run).
    LibreTranslate,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            ProviderKind::FreePublic => "Free public service (no key)",
            ProviderKind::BackendRelay => "My backend relay (key lives on the server)",
            ProviderKind::LibreTranslate => "LibreTranslate instance",
        }
    }

    pub fn all() -> [ProviderKind; 3] {
        [
            ProviderKind::FreePublic,
            ProviderKind::BackendRelay,
            ProviderKind::LibreTranslate,
        ]
    }
}

/// User preferences. Everything here is safe to write to disk: no translated
/// text and no captured application content is ever stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub hover_delay_secs: f32,
    pub target_language: String,
    pub source_language: String,
    pub provider: ProviderKind,
    pub backend_url: String,
    pub libretranslate_url: String,
    pub max_chars: usize,
    pub popup_offset_x: f32,
    pub popup_offset_y: f32,
    pub popup_max_width: f32,
    pub requests_per_minute: u32,
    pub cache_ttl_secs: u64,
    pub cache_capacity: usize,
    pub show_control_panel_on_start: bool,
    pub detect_source_language: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            hover_delay_secs: 0.8,
            target_language: "es".to_string(),
            source_language: "auto".to_string(),
            provider: ProviderKind::FreePublic,
            backend_url: "http://127.0.0.1:8787/translate".to_string(),
            libretranslate_url: "http://127.0.0.1:5000/translate".to_string(),
            max_chars: 1500,
            popup_offset_x: 18.0,
            popup_offset_y: 22.0,
            popup_max_width: 420.0,
            requests_per_minute: 30,
            cache_ttl_secs: 600,
            cache_capacity: 256,
            show_control_panel_on_start: true,
            detect_source_language: true,
        }
    }
}

impl Settings {
    /// Where the settings file lives: `%APPDATA%\HoverTranslate\settings.json`.
    pub fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("HoverTranslate").join("settings.json")
    }

    pub fn load() -> Settings {
        let path = Settings::path();
        let mut settings = Settings::default();
        if let Ok(text) = std::fs::read_to_string(&path) {
            // A file written by another program may start with a byte order
            // mark, which would stop the reading of the settings.
            let text = text.trim_start_matches('\u{feff}');
            if let Ok(parsed) = serde_json::from_str::<Settings>(text) {
                settings = parsed;
            }
        }
        settings.clamped()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Settings::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())
    }

    /// Keep values inside sane ranges so a hand-edited file cannot break the app.
    pub fn clamped(mut self) -> Settings {
        self.hover_delay_secs = self.hover_delay_secs.clamp(0.1, 5.0);
        self.max_chars = self.max_chars.clamp(50, 5000);
        self.popup_max_width = self.popup_max_width.clamp(200.0, 800.0);
        self.requests_per_minute = self.requests_per_minute.clamp(1, 240);
        self.cache_capacity = self.cache_capacity.clamp(16, 4096);
        self.cache_ttl_secs = self.cache_ttl_secs.clamp(10, 86_400);
        if self.target_language.trim().is_empty() {
            self.target_language = "es".to_string();
        }
        self
    }
}
