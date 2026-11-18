use super::{outcome, TranslateError, TranslationOutcome, TranslationProvider, TranslationRequest};
use crate::languages;
use crate::settings::ProviderKind;
use std::time::Instant;

/// A LibreTranslate instance (self-hosted or run by a provider).
pub struct LibreTranslateProvider {
    client: reqwest::blocking::Client,
    endpoint: String,
}

#[derive(serde::Serialize)]
struct LibreRequest<'a> {
    q: &'a str,
    source: &'a str,
    target: &'a str,
    format: &'static str,
}

impl LibreTranslateProvider {
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

impl TranslationProvider for LibreTranslateProvider {
    fn id(&self) -> &'static str {
        "libretranslate"
    }

    fn label(&self) -> String {
        format!("LibreTranslate ({})", self.endpoint)
    }

    fn translate(&self, request: &TranslationRequest) -> Result<TranslationOutcome, TranslateError> {
        let started = Instant::now();
        if self.endpoint.is_empty() {
            return Err(TranslateError::Rejected(
                "no LibreTranslate address configured".to_string(),
            ));
        }

        // LibreTranslate knows Chinese as `zh`, not as `zh-CN` or `zh-TW`.
        let source = languages::provider_source(ProviderKind::LibreTranslate, &request.source);
        let target = languages::provider_target(ProviderKind::LibreTranslate, &request.target);
        let payload = LibreRequest {
            q: &request.text,
            source: &source,
            target: &target,
            format: "text",
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
                "the instance is rate limiting requests".to_string(),
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
                .unwrap_or_else(|| format!("the instance answered {}", status.as_u16()));
            return Err(TranslateError::Rejected(detail));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| TranslateError::Rejected(format!("unexpected answer: {e}")))?;

        let translated = json
            .get("translatedText")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if translated.trim().is_empty() {
            return Err(TranslateError::Unsupported(
                "the instance returned no translation".to_string(),
            ));
        }

        let detected = json
            .get("detectedLanguage")
            .and_then(|v| v.get("language"))
            .and_then(|v| v.as_str())
            .map(languages::normalise_detected)
            .filter(|code| !code.is_empty());

        Ok(outcome(self.id(), translated, detected, started, false))
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
