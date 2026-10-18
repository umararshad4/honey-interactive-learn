//! Backend for the Google Gemini API (`generateContent` with JSON-schema output).

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{GenerateRequest, LlmBackend, parse_json_loose};

pub const DEFAULT_MODEL: &str = "gemini-3.8-flash";

pub struct GeminiApi {
    api_key: String,
    model: String,
    base_url: String,
}

impl GeminiApi {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.into()),
            base_url: std::env::var("GEMINI_BASE_URL")
                .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta".into())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    fn body(req: &GenerateRequest) -> Value {
        json!({
            "systemInstruction": {"parts": [{"text": req.system}]},
            "contents": [{"role": "user", "parts": [{"text": req.user}]}],
            "generationConfig": {
                "responseMimeType": "application/json",
                "responseJsonSchema": req.schema
            }
        })
    }
}

impl LlmBackend for GeminiApi {
    fn label(&self) -> String {
        format!("gemini:{}", self.model)
    }

    fn generate(&self, req: &GenerateRequest) -> Result<Value> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30 * 60)))
            .http_status_as_error(false)
            .build()
            .into();
        let model = self.model.trim_start_matches("models/");
        let mut resp = agent
            .post(format!("{}/models/{model}:generateContent", self.base_url))
            .header("x-goog-api-key", &self.api_key)
            .header("content-type", "application/json")
            .send_json(Self::body(req))
            .context("could not reach the Gemini API")?;
        let status = resp.status();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if !status.is_success() {
            let message = body["error"]["message"].as_str().unwrap_or(&text);
            bail!("Gemini API error {status}: {message}");
        }
        parse_json_loose(&candidate_text(&body)?)
    }
}

/// Extracts the first candidate's text, surfacing blocks and truncation.
pub fn candidate_text(body: &Value) -> Result<String> {
    if let Some(reason) = body["promptFeedback"]["blockReason"].as_str() {
        bail!("Gemini blocked the request ({reason})");
    }
    let candidate = &body["candidates"][0];
    match candidate["finishReason"].as_str() {
        Some("MAX_TOKENS") => bail!("Gemini stopped early (MAX_TOKENS); try a smaller depth"),
        Some(r @ ("SAFETY" | "RECITATION" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII")) => {
            bail!("Gemini declined to write this lesson ({r})")
        }
        _ => {}
    }
    let text: String = candidate["content"]["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["thought"] != true)
        .filter_map(|p| p["text"].as_str())
        .collect();
    if text.trim().is_empty() {
        bail!("Gemini returned no text");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_requests_json_schema_output() {
        let schema = json!({"type": "object"});
        let body = GeminiApi::body(&GenerateRequest {
            system: "sys",
            user: "hi",
            schema: &schema,
        });
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "sys");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "hi");
        assert_eq!(
            body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert_eq!(body["generationConfig"]["responseJsonSchema"], schema);
    }

    #[test]
    fn joins_parts_and_skips_thoughts() {
        let body = json!({"candidates": [{"finishReason": "STOP", "content": {"parts": [
            {"text": "thinking...", "thought": true},
            {"text": "{\"a\":"},
            {"text": "1}"}
        ]}}]});
        assert_eq!(candidate_text(&body).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn reports_blocks_and_truncation() {
        let blocked = json!({"promptFeedback": {"blockReason": "SAFETY"}});
        assert!(
            candidate_text(&blocked)
                .unwrap_err()
                .to_string()
                .contains("SAFETY")
        );
        let cut = json!({"candidates": [{"finishReason": "MAX_TOKENS", "content": {"parts": [{"text": "{"}]}}]});
        assert!(
            candidate_text(&cut)
                .unwrap_err()
                .to_string()
                .contains("MAX_TOKENS")
        );
        assert!(candidate_text(&json!({"candidates": []})).is_err());
    }
}
