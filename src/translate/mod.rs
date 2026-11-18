//! The translation layer.
//!
//! The desktop app only ever talks to the [`TranslationProvider`] trait, so a
//! different service can be plugged in without touching the UI, the hover
//! watcher or the popup.

pub mod backend;
pub mod cache;
pub mod libretranslate;
pub mod mymemory;
pub mod ratelimit;

use std::time::Instant;


#[derive(Debug, Clone)]
pub struct TranslationRequest {
    /// The text picked up under the pointer.
    pub text: String,
    /// "auto" means the provider should detect the source language.
    pub source: String,
    /// Target language code, e.g. "es".
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct TranslationOutcome {
    pub translated: String,
    /// Source language the provider detected, when it tells us.
    pub detected_source: Option<String>,
    /// Which provider produced this text.
    pub provider: String,
    /// True when the answer came from the in-memory cache.
    pub from_cache: bool,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone)]
pub enum TranslateError {
    /// No usable text was found under the pointer.
    Unsupported(String),
    /// The device is offline or the service could not be reached.
    Network(String),
    /// The service refused the request (quota, auth, bad request).
    Rejected(String),
    /// The local rate limiter said no.
    RateLimited(String),
}

impl std::fmt::Display for TranslateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslateError::Unsupported(m) => write!(f, "Nothing to translate: {m}"),
            TranslateError::Network(m) => write!(f, "No connection: {m}"),
            TranslateError::Rejected(m) => write!(f, "Service refused the request: {m}"),
            TranslateError::RateLimited(m) => write!(f, "Slow down: {m}"),
        }
    }
}

impl TranslateError {
    /// Short label shown inside the popup.
    pub fn kind(&self) -> &'static str {
        match self {
            TranslateError::Unsupported(_) => "unsupported",
            TranslateError::Network(_) => "offline",
            TranslateError::Rejected(_) => "rejected",
            TranslateError::RateLimited(_) => "rate limit",
        }
    }
}

/// A replaceable online translation service.
pub trait TranslationProvider: Send + Sync {
    /// Stable id, used in the cache key and in the UI.
    fn id(&self) -> &'static str;
    /// Human readable name shown in Settings.
    #[allow(dead_code)]
    fn label(&self) -> String;
    /// Translate one piece of text.
    fn translate(&self, request: &TranslationRequest) -> Result<TranslationOutcome, TranslateError>;
}

/// Cache key: the same text and target always give the same answer.
pub fn cache_key(provider: &str, request: &TranslationRequest) -> String {
    format!("{}\u{1}{}\u{1}{}", provider, request.target, request.text)
}

/// Turn a provider answer into an outcome, measuring how long it took.
pub fn outcome(
    provider: &str,
    translated: String,
    detected_source: Option<String>,
    started: Instant,
    from_cache: bool,
) -> TranslationOutcome {
    TranslationOutcome {
        translated,
        detected_source,
        provider: provider.to_string(),
        from_cache,
        elapsed_ms: started.elapsed().as_millis(),
    }
}

/// Split a text into pieces that a service will accept.
///
/// The free service counts the request in bytes, and an Arabic letter takes
/// two bytes and a Chinese letter three, so a long sentence in those languages
/// has to be cut into smaller pieces before it is sent. The pieces are cut at
/// a full stop, a space or, for Chinese and Japanese, between two characters.
pub fn split_for_request(text: &str, max_bytes: usize) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.len() <= max_bytes {
        return vec![trimmed.to_string()];
    }

    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut last_break: Option<usize> = None;

    for ch in trimmed.chars() {
        let length = ch.len_utf8();
        if !current.is_empty() && current.len() + length > max_bytes {
            let cut = last_break.filter(|cut| *cut > 0).unwrap_or(current.len());
            let piece = current[..cut].trim().to_string();
            let rest = current[cut..].trim_start().to_string();
            if !piece.is_empty() {
                pieces.push(piece);
            }
            current = rest;
            last_break = None;
        }

        if ch == '\n'
            || matches!(
                ch,
                '.' | '!' | '?' | '。' | '！' | '？' | '،' | '؛' | '؟' | '۔'
            )
            || ch.is_whitespace()
            || crate::languages::is_cjk(ch)
        {
            last_break = Some(current.len() + length);
        }
        current.push(ch);
    }

    let tail = current.trim();
    if !tail.is_empty() {
        pieces.push(tail.to_string());
    }
    pieces
}

/// Put translated pieces back together.
///
/// Chinese and Japanese are written without spaces between the words, so no
/// space is put between the pieces there.
pub fn join_pieces(pieces: &[String]) -> String {
    let mut out = String::new();
    for piece in pieces {
        if out.is_empty() {
            out.push_str(piece);
            continue;
        }
        let previous = out.chars().last().unwrap_or(' ');
        let next = piece.chars().next().unwrap_or(' ');
        if crate::languages::is_cjk(previous) || crate::languages::is_cjk(next) {
            out.push_str(piece);
        } else {
            out.push(' ');
            out.push_str(piece);
        }
    }
    out
}

/// Decode the handful of HTML entities some free services return.
///
/// The text can be in any language, so the scan walks over characters and
/// never cuts a multi byte letter in half.
pub fn decode_entities(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'&' {
            if let Some(offset) = input[index + 1..].find(';') {
                let end = index + 1 + offset;
                if end - index <= 12 {
                    let entity = &input[index + 1..end];
                    if let Some(replacement) = decode_one(entity) {
                        out.push_str(&replacement);
                        index = end + 1;
                        continue;
                    }
                }
            }
        }
        let ch = input[index..].chars().next().unwrap_or('\u{fffd}');
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn decode_one(entity: &str) -> Option<String> {
    let replacement = match entity {
        "amp" => "&".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        "nbsp" => " ".to_string(),
        "hellip" => "\u{2026}".to_string(),
        "mdash" => "\u{2014}".to_string(),
        "ndash" => "\u{2013}".to_string(),
        "rsquo" => "\u{2019}".to_string(),
        "lsquo" => "\u{2018}".to_string(),
        "ldquo" => "\u{201c}".to_string(),
        "rdquo" => "\u{201d}".to_string(),
        _ => {
            // Numbers, such as &#39; or &#x2019;. The name that is handed
            // over starts after the "&", so it begins with a "#".
            if let Some(digits) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
                return u32::from_str_radix(digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(|ch| ch.to_string());
            }
            if let Some(digits) = entity.strip_prefix('#') {
                return digits
                    .parse::<u32>()
                    .ok()
                    .and_then(char::from_u32)
                    .map(|ch| ch.to_string());
            }
            return None;
        }
    };
    Some(replacement)
}

/// The free service sometimes answers with a notice about the word instead of
/// a translation, for example `[ترجمة المصطلح: Hello]`. The notice is not a
/// translation, so it is taken off.
pub fn strip_service_notice(text: &str, source: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let (_, after) = inner.split_once(':')?;
    let after = after.trim();
    let source = source.trim();
    if after.is_empty() {
        return None;
    }
    if after.eq_ignore_ascii_case(source) {
        // The service only repeated the word: there is no translation.
        return Some(String::new());
    }
    // Something else inside brackets: keep the part after the colon.
    let cleaned = after.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_next_to_arabic_do_not_break_the_text() {
        let input = "المصنع & الشركة &#39;جديد&#39;";
        let decoded = decode_entities(input);
        assert!(decoded.contains('&'));
        assert!(decoded.contains('\''));
        assert!(decoded.starts_with("المصنع"));
    }

    #[test]
    fn a_long_arabic_text_is_cut_into_pieces() {
        let text = "الكلمة ".repeat(120);
        let pieces = split_for_request(&text, 200);
        assert!(pieces.len() > 1);
        for piece in &pieces {
            assert!(piece.len() <= 220, "piece too long: {}", piece.len());
        }
    }

    #[test]
    fn chinese_text_is_cut_between_characters() {
        let text = "今天天气很好".repeat(40);
        let pieces = split_for_request(&text, 100);
        assert!(pieces.len() > 1);
        for piece in &pieces {
            assert!(piece.len() <= 110);
        }
        assert_eq!(join_pieces(&pieces), text);
    }

    #[test]
    fn the_notice_of_the_service_is_taken_off() {
        assert_eq!(
            strip_service_notice("[ترجمة المصطلح: Hello]", "Hello"),
            Some(String::new())
        );
        assert_eq!(strip_service_notice("مرحبا", "Hello"), None);
    }
}
