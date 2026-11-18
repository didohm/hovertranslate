//! The languages the app can translate into.
//!
//! Every service names the languages a little differently: LibreTranslate wants
//! `zh` where MyMemory wants `zh-CN`, and MyMemory calls "detect the source"
//! `autodetect`. The app keeps its own codes in the settings and asks this
//! module to turn them into the codes a service understands.

use crate::settings::ProviderKind;

/// A language the user can pick as the translation target.
pub struct Language {
    /// The code the app stores in the settings.
    pub code: &'static str,
    /// The name shown in the settings.
    pub name: &'static str,
    /// A short sentence in this language, used to show how the result will
    /// look before anything is hovered.
    pub sample: &'static str,
    /// True for languages that are written from right to left. The direction
    /// used while drawing is detected from the text itself by `bidi.rs`.
    #[allow(dead_code)]
    pub rtl: bool,
}

pub const LANGUAGES: &[Language] = &[
    Language { code: "ar", name: "Arabic", sample: "ضع المؤشر على أي نص لقراءته بلغتك", rtl: true },
    Language { code: "zh-CN", name: "Chinese (Simplified)", sample: "您好，您今天过得怎么样？", rtl: false },
    Language { code: "zh-TW", name: "Chinese (Traditional)", sample: "您好，您今天過得好嗎？", rtl: false },
    Language { code: "cs", name: "Czech", sample: "Ahoj, světe.", rtl: false },
    Language { code: "nl", name: "Dutch", sample: "Hallo, wereld.", rtl: false },
    Language { code: "en", name: "English", sample: "Hello, how are you today?", rtl: false },
    Language { code: "fr", name: "French", sample: "Bonjour, comment allez-vous aujourd'hui ?", rtl: false },
    Language { code: "de", name: "German", sample: "Hallo, wie geht es dir heute?", rtl: false },
    Language { code: "el", name: "Greek", sample: "Γειά σου, κόσμε.", rtl: false },
    Language { code: "he", name: "Hebrew", sample: "שלום, מה שלומך היום?", rtl: true },
    Language { code: "hi", name: "Hindi", sample: "नमस्ते, आप कैसे हैं?", rtl: false },
    Language { code: "hu", name: "Hungarian", sample: "Szia, világ!", rtl: false },
    Language { code: "id", name: "Indonesian", sample: "Halo, dunia.", rtl: false },
    Language { code: "it", name: "Italian", sample: "Ciao, mondo.", rtl: false },
    Language { code: "ja", name: "Japanese", sample: "こんにちは、世界。", rtl: false },
    Language { code: "ko", name: "Korean", sample: "안녕하세요, 세상.", rtl: false },
    Language { code: "lv", name: "Latvian", sample: "Sveiki, pasaule!", rtl: false },
    Language { code: "lt", name: "Lithuanian", sample: "Sveiki, pasauli!", rtl: false },
    Language { code: "pl", name: "Polish", sample: "Cześć, świecie.", rtl: false },
    Language { code: "pt", name: "Portuguese", sample: "Olá, mundo!", rtl: false },
    Language { code: "ro", name: "Romanian", sample: "Salut, lume!", rtl: false },
    Language { code: "ru", name: "Russian", sample: "Привет, мир!", rtl: false },
    Language { code: "es", name: "Spanish", sample: "¡Hola, mundo!", rtl: false },
    Language { code: "sv", name: "Swedish", sample: "Hej, världen!", rtl: false },
    Language { code: "th", name: "Thai", sample: "สวัสดีชาวโลก", rtl: false },
    Language { code: "tr", name: "Turkish", sample: "Merhaba dünya!", rtl: false },
    Language { code: "uk", name: "Ukrainian", sample: "Привіт, світе!", rtl: false },
    Language { code: "vi", name: "Vietnamese", sample: "Xin chào thế giới!", rtl: false },
];

/// The code used when the source language should be detected.
pub const AUTO: &str = "auto";

pub fn find(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|language| language.code == code)
}

/// The name of a language, or the code itself when it is unknown.
pub fn language_label(code: &str) -> String {
    find(code)
        .map(|language| language.name.to_string())
        .unwrap_or_else(|| code.to_string())
}

/// A short sentence in the language, used for the preview of the popup.
pub fn sample_for(code: &str) -> String {
    find(code)
        .map(|language| language.sample.to_string())
        .unwrap_or_else(|| "Hello, how are you today?".to_string())
}

/// True when the language is written from right to left.
#[allow(dead_code)]
pub fn is_rtl(code: &str) -> bool {
    find(code).map(|language| language.rtl).unwrap_or(false)
}

/// The code the service expects for the target language.
///
/// The app keeps `zh-CN` and `zh-TW` apart because MyMemory can tell them
/// apart, while LibreTranslate only knows `zh`.
pub fn provider_target(provider: ProviderKind, code: &str) -> String {
    match provider {
        ProviderKind::LibreTranslate => match code {
            "zh-CN" | "zh-TW" => "zh".to_string(),
            other => other.to_string(),
        },
        // The relay speaks the app's own codes and translates them itself.
        ProviderKind::BackendRelay | ProviderKind::FreePublic => code.to_string(),
    }
}

/// The code the service expects for the source language.
pub fn provider_source(provider: ProviderKind, source: &str) -> String {
    if source != AUTO {
        return source.to_string();
    }
    match provider {
        // MyMemory calls the automatic source "autodetect".
        ProviderKind::FreePublic => "autodetect".to_string(),
        ProviderKind::LibreTranslate | ProviderKind::BackendRelay => AUTO.to_string(),
    }
}

/// True when the service understands the language at all.
pub fn provider_supports(provider: ProviderKind, code: &str) -> bool {
    match provider {
        ProviderKind::LibreTranslate => !matches!(code, "zh-TW"),
        ProviderKind::FreePublic | ProviderKind::BackendRelay => true,
    }
}

/// Normalise a language code that came back from a service.
///
/// Services like to answer with `en-GB` or `zh-Hans`; the settings only know
/// the short codes.
pub fn normalise_detected(code: &str) -> String {
    let code = code.trim();
    if code.is_empty() {
        return String::new();
    }
    let lower = code.to_ascii_lowercase();
    match lower.as_str() {
        "zh-hans" | "zh-cn" | "zh" | "chinese" | "chinese simplified" => "zh-CN".to_string(),
        "zh-hant" | "zh-tw" | "zh-hk" | "chinese traditional" => "zh-TW".to_string(),
        _ => {
            let short = lower.split(['-', '_']).next().unwrap_or(&lower);
            short.to_string()
        }
    }
}

/// True when the text itself reads from right to left.
///
/// The direction comes from the first strong letter (the same rule as
/// `unicode-bidi: plaintext`), so an English sentence that happens to contain
/// one Arabic word is still laid out left to right.
#[allow(dead_code)]
pub fn text_is_rtl(text: &str) -> bool {
    crate::bidi::base_is_rtl(text)
}

/// True for characters of right-to-left scripts.
pub fn is_rtl_char(ch: char) -> bool {
    matches!(ch as u32,
        0x0590..=0x05FF // Hebrew
        | 0x0600..=0x06FF // Arabic
        | 0x0700..=0x074F // Syriac
        | 0x0750..=0x077F // Arabic supplement
        | 0x08A0..=0x08FF // Arabic extended-A
        | 0xFB1D..=0xFDFF // Hebrew and Arabic presentation forms
        | 0xFE70..=0xFEFF // Arabic presentation forms-B
    )
}

/// True for characters that are written without a space between words.
pub fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x2E80..=0x2FDF // radicals
        | 0x3000..=0x303F // punctuation
        | 0x3040..=0x30FF // kana
        | 0x3400..=0x4DBF // extension A
        | 0x4E00..=0x9FFF // unified ideographs
        | 0xF900..=0xFAFF // compatibility ideographs
        | 0xFF00..=0xFFEF // half width and full width forms
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_codes_differ_per_service() {
        assert_eq!(provider_target(ProviderKind::FreePublic, "zh-CN"), "zh-CN");
        assert_eq!(provider_target(ProviderKind::LibreTranslate, "zh-CN"), "zh");
        assert_eq!(provider_target(ProviderKind::LibreTranslate, "zh-TW"), "zh");
    }

    #[test]
    fn automatic_source_is_named_per_service() {
        assert_eq!(provider_source(ProviderKind::FreePublic, AUTO), "autodetect");
        assert_eq!(provider_source(ProviderKind::LibreTranslate, AUTO), AUTO);
    }

    #[test]
    fn detected_codes_are_shortened() {
        assert_eq!(normalise_detected("en-GB"), "en");
        assert_eq!(normalise_detected("zh-Hans"), "zh-CN");
        assert_eq!(normalise_detected("zh-TW"), "zh-TW");
        assert_eq!(normalise_detected("  "), "");
    }

    #[test]
    fn arabic_and_hebrew_are_right_to_left() {
        assert!(text_is_rtl("مرحبا بالعالم"));
        assert!(!text_is_rtl("Hello world"));
        assert!(is_rtl("ar"));
        assert!(!is_rtl("zh-CN"));
    }

    #[test]
    fn direction_comes_from_the_first_strong_letter() {
        assert!(text_is_rtl("ضع المؤشر على أي نص"));
        assert!(!text_is_rtl("Rest the pointer on any text"));
        assert!(!text_is_rtl("Hello مرحبا"));
    }
}
