//! Backend for the Anthropic Messages API over raw HTTP (no official Rust SDK).
//!
//! Streams the response so long lessons never hit HTTP timeouts, asks for
//! structured JSON output, and opts into server-side refusal fallbacks.

use std::io::{BufRead, BufReader, Read};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{GenerateRequest, LlmBackend, parse_json_loose};

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
const DEFAULT_EFFORT: &str = "high";
const MAX_TOKENS: u32 = 32_000;

pub struct AnthropicApi {
    api_key: String,
    model: String,
    effort: String,
    base_url: String,
}

impl AnthropicApi {
    pub fn new(api_key: String, model: Option<String>, effort: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.into()),
            effort: effort.unwrap_or_else(|| DEFAULT_EFFORT.into()),
            base_url: std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".into())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    fn body(&self, req: &GenerateRequest) -> Value {
        json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "stream": true,
            "system": req.system,
            "messages": [{"role": "user", "content": req.user}],
            "output_config": {
                "effort": self.effort,
                "format": {"type": "json_schema", "schema": req.schema}
            },
            "fallbacks": "default"
        })
    }
}

impl LlmBackend for AnthropicApi {
    fn label(&self) -> String {
        format!("api:{}", self.model)
    }

    fn generate(&self, req: &GenerateRequest) -> Result<Value> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30 * 60)))
            .http_status_as_error(false)
            .build()
            .into();
        let resp = agent
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "server-side-fallback-2026-07-01")
            .header("content-type", "application/json")
            .send_json(self.body(req))
            .context("could not reach the Anthropic API")?;

        let status = resp.status();
        let mut body = resp.into_body();
        if !status.is_success() {
            let text = body.read_to_string().unwrap_or_default();
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or(text);
            bail!("Anthropic API error {status}: {message}");
        }
        let text = parse_sse_stream(body.as_reader())?;
        parse_json_loose(&text)
    }
}

/// Reads a Messages API SSE stream and returns the concatenated text output.
///
/// Fails on `error` events and on `refusal` / `max_tokens` stop reasons.
pub fn parse_sse_stream(reader: impl Read) -> Result<String> {
    let mut text = String::new();
    let mut stop_reason: Option<String> = None;
    for line in BufReader::new(reader).lines() {
        let line = line.context("connection dropped while streaming")?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
            continue;
        };
        match event["type"].as_str() {
            Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                text.push_str(event["delta"]["text"].as_str().unwrap_or_default());
            }
            Some("message_delta") => {
                if let Some(r) = event["delta"]["stop_reason"].as_str() {
                    stop_reason = Some(r.to_string());
                }
            }
            Some("error") => {
                let msg = event["error"]["message"]
                    .as_str()
                    .unwrap_or("unknown error");
                bail!("Anthropic API stream error: {msg}");
            }
            _ => {}
        }
    }
    match stop_reason.as_deref() {
        Some("refusal") => bail!("the model declined to write this lesson (refusal)"),
        Some("max_tokens") => bail!("the lesson was cut off (max_tokens); try a smaller depth"),
        _ if text.trim().is_empty() => bail!("the API stream ended without any text"),
        _ => Ok(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sse(events: &[&str]) -> String {
        events
            .iter()
            .map(|e| format!("event: x\ndata: {e}\n\n"))
            .collect()
    }

    #[test]
    fn concatenates_text_deltas_across_blocks() {
        let stream = sse(&[
            r#"{"type":"message_start","message":{"id":"m"}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"{\"a\":"}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"1}"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#,
            r#"{"type":"message_stop"}"#,
        ]);
        assert_eq!(parse_sse_stream(stream.as_bytes()).unwrap(), r#"{"a":1}"#);
    }

    #[test]
    fn reports_refusals_truncation_and_errors() {
        let refusal = sse(&[r#"{"type":"message_delta","delta":{"stop_reason":"refusal"}}"#]);
        assert!(
            parse_sse_stream(refusal.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("refusal")
        );
        let cut = sse(&[
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"{"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens"}}"#,
        ]);
        assert!(
            parse_sse_stream(cut.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("max_tokens")
        );
        let err = sse(&[
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        ]);
        assert!(
            parse_sse_stream(err.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("Overloaded")
        );
    }

    #[test]
    fn request_body_uses_structured_output_and_fallbacks() {
        let api = AnthropicApi::new("k".into(), None, None);
        let schema = json!({"type": "object"});
        let body = api.body(&GenerateRequest {
            system: "sys",
            user: "hi",
            schema: &schema,
        });
        assert_eq!(body["model"], DEFAULT_MODEL);
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(body["output_config"]["effort"], "high");
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["stream"], true);
    }
}
