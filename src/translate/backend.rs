use super::{
    decode_entities, outcome, TranslateError, TranslationOutcome, TranslationProvider,
    TranslationRequest,
};
use crate::languages;
use std::time::Instant;

/// Talks to a backend that the person runs (or their team runs).
///
/// The provider API key lives on that server, so nothing secret is ever
/// shipped inside the desktop executable.
pub struct BackendProvider {
    client: reqwest::blocking::Client,
    endpoint: String,
}

#[derive(serde::Serialize)]
struct BackendRequest<'a> {
    text: &'a str,
    source: &'a str,
    target: &'a str,
}

impl BackendProvider {
    pub fn new(endpoint: &str) -> Result<Self, TranslateError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("HoverTranslate/0.1 (desktop hover translator)")
            .build()
            .map_err(|e| TranslateError::Network(e.to_string()))?;
        Ok(Self {
            client,
            endpoint: endpoint.trim().to_string(),
        })
    }
}

impl TranslationProvider for BackendProvider {
    fn id(&self) -> &'static str {
        "backend"
    }

    fn label(&self) -> String {
        format!("Backend relay ({})", self.endpoint)
    }

    fn translate(&self, request: &TranslationRequest) -> Result<TranslationOutcome, TranslateError> {
        let started = Instant::now();
        if self.endpoint.is_empty() {
            return Err(TranslateError::Rejected(
                "no backend address configured".to_string(),
            ));
        }

        // The relay speaks the codes of the app and turns them into the codes
        // of the service it uses.
        let payload = BackendRequest {
            text: &request.text,
            source: &request.source,
            target: &request.target,
        };

        let response = self
            .client
            .post(&self.endpoint)
            .json(&payload)
            .send()
            .map_err(|e| TranslateError::Network(clean(&e.to_string())))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| TranslateError::Network(e.to_string()))?;

        if status.as_u16() == 429 {
            return Err(TranslateError::RateLimited(
                "the backend is rate limiting requests".to_string(),
            ));
        }
        if !status.is_success() {
            let detail = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|json| {
                    json.get("error")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| format!("the backend answered {}", status.as_u16()));
            return Err(TranslateError::Rejected(detail));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| TranslateError::Rejected(format!("unexpected answer: {e}")))?;

        let translated = json
            .get("translation")
            .or_else(|| json.get("translatedText"))
            .or_else(|| json.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if translated.trim().is_empty() {
            return Err(TranslateError::Unsupported(
                "the backend returned no translation".to_string(),
            ));
        }

        let detected = json
            .get("detected_source")
            .or_else(|| json.get("detectedSource"))
            .or_else(|| json.get("source"))
            .and_then(|v| v.as_str())
            .map(languages::normalise_detected)
            .filter(|code| !code.is_empty());

        Ok(outcome(
            self.id(),
            decode_entities(&translated).trim().to_string(),
            detected,
            started,
            false,
        ))
    }
}

fn clean(message: &str) -> String {
    let cleaned = message.replace('\n', " ");
    if cleaned.chars().count() > 120 {
        let head: String = cleaned.chars().take(120).collect();
        format!("{head}...")
    } else {
        cleaned
    }
}
