//! Backend for the OpenAI Responses API with strict JSON-schema output.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{GenerateRequest, LlmBackend, parse_json_loose};

pub const DEFAULT_MODEL: &str = "gpt-6-astra";

pub struct OpenAiApi {
    api_key: String,
    model: String,
    effort: Option<String>,
    base_url: String,
}

impl OpenAiApi {
    pub fn new(api_key: String, model: Option<String>, effort: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.into()),
            effort,
            base_url: std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    fn body(&self, req: &GenerateRequest) -> Value {
        let mut body = json!({
            "model": self.model,
            "instructions": req.system,
            "input": req.user,
            "text": {
                "format": {"type": "json_schema", "name": "lesson", "schema": req.schema, "strict": true}
            }
        });
        if let Some(effort) = &self.effort {
            body["reasoning"] = json!({"effort": effort});
        }
        body
    }
}

impl LlmBackend for OpenAiApi {
    fn label(&self) -> String {
        format!("openai:{}", self.model)
    }

    fn generate(&self, req: &GenerateRequest) -> Result<Value> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30 * 60)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut resp = agent
            .post(format!("{}/responses", self.base_url))
            .header("authorization", &format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .send_json(self.body(req))
            .context("could not reach the OpenAI API")?;
        let status = resp.status();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if !status.is_success() {
            let message = body["error"]["message"].as_str().unwrap_or(&text);
            bail!("OpenAI API error {status}: {message}");
        }
        parse_json_loose(&response_text(&body)?)
    }
}

/// Extracts the output text of a Responses API result, surfacing refusals and truncation.
pub fn response_text(body: &Value) -> Result<String> {
    if body["status"] == "incomplete" {
        let reason = body["incomplete_details"]["reason"]
            .as_str()
            .unwrap_or("unknown");
        bail!("OpenAI stopped early ({reason}); try a smaller depth");
    }
    let mut text = String::new();
    for item in body["output"].as_array().into_iter().flatten() {
        if item["type"] != "message" {
            continue;
        }
        for part in item["content"].as_array().into_iter().flatten() {
            match part["type"].as_str() {
                Some("output_text") => text.push_str(part["text"].as_str().unwrap_or_default()),
                Some("refusal") => bail!(
                    "the model declined to write this lesson: {}",
                    part["refusal"].as_str().unwrap_or("refusal")
                ),
                _ => {}
            }
        }
    }
    if text.trim().is_empty() {
        bail!("OpenAI returned no text");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_uses_strict_json_schema_and_optional_effort() {
        let schema = json!({"type": "object"});
        let req = GenerateRequest {
            system: "sys",
            user: "hi",
            schema: &schema,
        };
        let api = OpenAiApi::new("k".into(), None, None);
        let body = api.body(&req);
        assert_eq!(body["model"], DEFAULT_MODEL);
        assert_eq!(body["instructions"], "sys");
        assert_eq!(body["text"]["format"]["type"], "json_schema");
        assert_eq!(body["text"]["format"]["strict"], true);
        assert!(body.get("reasoning").is_none());
        let api = OpenAiApi::new("k".into(), Some("gpt-6-luna".into()), Some("low".into()));
        let body = api.body(&req);
        assert_eq!(body["model"], "gpt-6-luna");
        assert_eq!(body["reasoning"]["effort"], "low");
    }

    #[test]
    fn reads_output_text_skipping_reasoning_items() {
        let body = json!({
            "status": "completed",
            "output": [
                {"type": "reasoning", "summary": []},
                {"type": "message", "content": [{"type": "output_text", "text": "{\"a\":1}"}]}
            ]
        });
        assert_eq!(response_text(&body).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn reports_refusals_and_truncation() {
        let refusal = json!({"status": "completed", "output": [
            {"type": "message", "content": [{"type": "refusal", "refusal": "No."}]}
        ]});
        assert!(
            response_text(&refusal)
                .unwrap_err()
                .to_string()
                .contains("No.")
        );
        let cut =
            json!({"status": "incomplete", "incomplete_details": {"reason": "max_output_tokens"}});
        assert!(
            response_text(&cut)
                .unwrap_err()
                .to_string()
                .contains("max_output_tokens")
        );
        assert!(response_text(&json!({"output": []})).is_err());
    }
}
