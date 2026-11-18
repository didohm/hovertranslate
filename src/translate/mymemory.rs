use super::{
    decode_entities, join_pieces, outcome, split_for_request, strip_service_notice, TranslateError,
    TranslationOutcome, TranslationProvider, TranslationRequest,
};
use crate::languages;
use crate::settings::ProviderKind;
use std::time::Instant;

/// The free, key-less public service used for the first release.
///
/// The request carries nothing but the text that has to be translated, which
/// keeps the "send only what is needed" privacy rule.
pub struct MyMemoryProvider {
    client: reqwest::blocking::Client,
}

/// The service refuses anything longer than this, and it counts bytes: an
/// Arabic letter is two bytes and a Chinese letter three.
const MAX_QUERY_BYTES: usize = 450;

impl MyMemoryProvider {
    pub fn new() -> Result<Self, TranslateError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .user_agent("HoverTranslate/0.1 (desktop hover translator)")
            .build()
            .map_err(|e| TranslateError::Network(e.to_string()))?;
        Ok(Self { client })
    }

    /// Ask the service about one piece of text.
    fn translate_one(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<(String, Option<String>), TranslateError> {
        let lang_pair = format!("{}|{}", source, target);
        let url = "https://api.mymemory.translated.net/get";

        let response = self
            .client
            .get(url)
            .query(&[("q", text), ("langpair", lang_pair.as_str())])
            .send()
            .map_err(|e| TranslateError::Network(short_error(&e.to_string())))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| TranslateError::Network(e.to_string()))?;

        if !status.is_success() {
            return Err(TranslateError::Rejected(format!(
                "the service answered {}",
                status.as_u16()
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| TranslateError::Rejected(format!("unexpected answer: {e}")))?;

        let status_code = json
            .get("responseStatus")
            .and_then(|v| v.as_i64())
            .unwrap_or(200);
        let details = json
            .get("responseDetails")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let translated = json
            .get("responseData")
            .and_then(|d| d.get("translatedText"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if status_code >= 400 {
            return Err(classify_error(&details, &translated));
        }

        let detected = json
            .get("responseData")
            .and_then(|d| d.get("detectedLanguage"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                json.get("responseData")
                    .and_then(|d| d.get("detectedSourceLanguage"))
                    .and_then(|v| v.as_str())
            })
            .map(languages::normalise_detected)
            .filter(|code| !code.is_empty());

        let cleaned = decode_entities(&translated).trim().to_string();
        if cleaned.is_empty() {
            return Err(TranslateError::Unsupported(
                "the service returned no translation".to_string(),
            ));
        }

        // The service sometimes answers with a notice about the word instead
        // of a translation, for example in Arabic: [ترجمة المصطلح: Hello].
        if let Some(stripped) = strip_service_notice(&cleaned, text) {
            if stripped.is_empty() {
                return Err(TranslateError::Unsupported(
                    "the service has no translation for this word".to_string(),
                ));
            }
            return Ok((stripped, detected));
        }

        Ok((cleaned, detected))
    }
}

impl TranslationProvider for MyMemoryProvider {
    fn id(&self) -> &'static str {
        "mymemory"
    }

    fn label(&self) -> String {
        "free public service".to_string()
    }

    fn translate(&self, request: &TranslationRequest) -> Result<TranslationOutcome, TranslateError> {
        let started = Instant::now();
        let source = languages::provider_source(ProviderKind::FreePublic, &request.source);
        let target = languages::provider_target(ProviderKind::FreePublic, &request.target);

        // A long text is cut into pieces the service will take.
        let pieces = split_for_request(&request.text, MAX_QUERY_BYTES);
        if pieces.is_empty() {
            return Err(TranslateError::Unsupported(
                "there was nothing to translate".to_string(),
            ));
        }

        let mut answers: Vec<String> = Vec::new();
        let mut detected: Option<String> = None;
        for piece in &pieces {
            let (answer, language) = self.translate_one(piece, &source, &target)?;
            answers.push(answer);
            if detected.is_none() {
                detected = language;
            }
        }

        Ok(outcome(
            self.id(),
            join_pieces(&answers),
            detected,
            started,
            false,
        ))
    }
}

/// Turn the answer of the service into the right kind of error.
fn classify_error(details: &str, translated: &str) -> TranslateError {
    let text = format!("{} {}", details, translated).to_ascii_uppercase();

    if text.contains("QUERY LENGTH LIMIT") {
        return TranslateError::Rejected(
            "the text was too long for the free service".to_string(),
        );
    }
    if text.contains("INVALID SOURCE LANGUAGE") || text.contains("INVALID TARGET LANGUAGE") {
        return TranslateError::Rejected(
            "the service does not know that language pair".to_string(),
        );
    }
    if text.contains("YOU USED ALL AVAILABLE FREE TRANSLATIONS")
        || text.contains("QUOTA")
        || text.contains("LIMIT EXCEEDED")
    {
        return TranslateError::RateLimited(
            "the free daily quota of the service is used up".to_string(),
        );
    }
    if text.contains("NO QUERY SPECIFIED") {
        return TranslateError::Unsupported("there was nothing to translate".to_string());
    }

    let detail = details.trim();
    if detail.is_empty() {
        TranslateError::Rejected("the service refused the request".to_string())
    } else {
        TranslateError::Rejected(detail.to_string())
    }
}

fn short_error(message: &str) -> String {
    let cleaned = message.replace('\n', " ");
    if cleaned.chars().count() > 120 {
        let head: String = cleaned.chars().take(120).collect();
        format!("{head}...")
    } else {
        cleaned
    }
}
