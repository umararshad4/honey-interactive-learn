//! Backend that shells out to Claude Code (`claude -p`), reusing its login.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::{GenerateRequest, LlmBackend, parse_json_loose};

pub struct ClaudeCli {
    model: Option<String>,
}

impl ClaudeCli {
    pub fn new(model: Option<String>) -> Self {
        Self { model }
    }
}

impl LlmBackend for ClaudeCli {
    fn label(&self) -> String {
        match &self.model {
            Some(m) => format!("claude-cli:{m}"),
            None => "claude-cli".into(),
        }
    }

    fn generate(&self, req: &GenerateRequest) -> Result<Value> {
        let schema = serde_json::to_string(req.schema)?;
        let mut cmd = Command::new("claude");
        cmd.args([
            "-p",
            "--output-format",
            "json",
            "--json-schema",
            &schema,
            "--system-prompt",
            req.system,
            // A pure text job: no tools, MCP servers, hooks, plugins or saved session.
            "--tools",
            "",
            "--strict-mcp-config",
            "--setting-sources",
            "",
            "--disable-slash-commands",
            "--no-session-persistence",
        ]);
        if let Some(model) = &self.model {
            cmd.args(["--model", model]);
        }
        // Run outside any project so no CLAUDE.md is picked up.
        cmd.current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "`claude` is not on PATH. Install Claude Code or set ANTHROPIC_API_KEY."
                )
            } else {
                anyhow::Error::new(e).context("failed to start `claude`")
            }
        })?;
        let mut stdin = child.stdin.take().context("no stdin for claude")?;
        let prompt = req.user.to_string();
        // Write on another thread so a large prompt cannot deadlock against stdout.
        let writer = std::thread::spawn(move || stdin.write_all(prompt.as_bytes()));
        let output = child.wait_with_output().context("claude did not finish")?;
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("prompt writer panicked"))?
            .context("failed to send the prompt to claude")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("claude exited with {}: {}", output.status, stderr.trim());
        }
        parse_cli_result(&stdout)
    }
}

/// Extracts the structured result from `claude -p --output-format json`.
pub fn parse_cli_result(stdout: &str) -> Result<Value> {
    let envelope: Value =
        serde_json::from_str(stdout.trim()).context("claude printed something other than JSON")?;
    let result_text = envelope.get("result").and_then(Value::as_str).unwrap_or("");
    if envelope.get("is_error").and_then(Value::as_bool) == Some(true) {
        let reason = if result_text.is_empty() {
            envelope
                .get("subtype")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        } else {
            result_text
        };
        bail!("claude reported an error: {reason}");
    }
    match envelope.get("structured_output") {
        Some(v) if v.is_object() => Ok(v.clone()),
        _ => parse_json_loose(result_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_structured_output() {
        let out = r#"{"type":"result","is_error":false,"result":"{\"title\":\"A\"}","structured_output":{"title":"B"}}"#;
        assert_eq!(parse_cli_result(out).unwrap()["title"], "B");
    }

    #[test]
    fn falls_back_to_result_text() {
        let out =
            r#"{"type":"result","is_error":false,"result":"```json\n{\"title\":\"A\"}\n```"}"#;
        assert_eq!(parse_cli_result(out).unwrap()["title"], "A");
    }

    #[test]
    fn surfaces_errors() {
        let out = r#"{"type":"result","is_error":true,"result":"Not logged in"}"#;
        let err = parse_cli_result(out).unwrap_err().to_string();
        assert!(err.contains("Not logged in"), "{err}");
    }
}
