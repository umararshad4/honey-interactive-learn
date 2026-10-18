//! User settings shared by the CLI and the desktop app (`<data dir>/settings.json`).
//!
//! Environment variables always win over the file, so shell users can keep
//! keys in their profile while desktop users set them in the app.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::llm::BackendChoice;

/// `$HIL_HOME`, or the platform data dir
/// (`~/Library/Application Support/honey-interactive-learn` on macOS).
pub fn data_dir() -> Result<PathBuf> {
    match std::env::var_os("HIL_HOME") {
        Some(home) => Ok(PathBuf::from(home)),
        None => Ok(dirs::data_dir()
            .context("could not find a data directory for this platform")?
            .join("honey-interactive-learn")),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub backend: BackendChoice,
    /// Reasoning effort for API providers that support it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gemini_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_cli_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gemini_model: Option<String>,
}

/// Names accepted by `hil config set/unset`, with a short description.
pub const FIELDS: &[(&str, &str)] = &[
    ("backend", "auto, claude-cli, anthropic, openai or gemini"),
    (
        "effort",
        "reasoning effort: low, medium, high, xhigh or max",
    ),
    ("anthropic-key", "Anthropic API key"),
    ("openai-key", "OpenAI API key"),
    ("gemini-key", "Google Gemini API key"),
    (
        "claude-cli-model",
        "model for the claude CLI (e.g. opus, sonnet)",
    ),
    ("anthropic-model", "Anthropic model id"),
    ("openai-model", "OpenAI model id"),
    ("gemini-model", "Gemini model id"),
];

impl Settings {
    pub fn path() -> Result<PathBuf> {
        Ok(data_dir()?.join("settings.json"))
    }

    /// Loads settings; a missing file means defaults.
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path()?)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("{} is not valid settings JSON", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("could not read {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    /// Writes the file readable by the current user only (it may hold API keys).
    ///
    /// The temp file is created with mode 0600 before any bytes are written
    /// (`tempfile` uses `O_EXCL`, so it never follows a planted symlink), then
    /// renamed into place atomically.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        tmp.write_all(&serde_json::to_vec_pretty(self)?)?;
        tmp.as_file().sync_all()?;
        tmp.persist(path)
            .with_context(|| format!("could not save {}", path.display()))?;
        Ok(())
    }

    /// Sets a field by its `hil config` name. An empty value clears it.
    pub fn set(&mut self, field: &str, value: &str) -> Result<(), String> {
        let value = value.trim();
        let opt = (!value.is_empty()).then(|| value.to_string());
        match field {
            "backend" => {
                self.backend = if value.is_empty() {
                    BackendChoice::Auto
                } else {
                    value.parse()?
                }
            }
            "effort" => self.effort = opt,
            "anthropic-key" => self.anthropic_api_key = opt,
            "openai-key" => self.openai_api_key = opt,
            "gemini-key" => self.gemini_api_key = opt,
            "claude-cli-model" => self.claude_cli_model = opt,
            "anthropic-model" => self.anthropic_model = opt,
            "openai-model" => self.openai_model = opt,
            "gemini-model" => self.gemini_model = opt,
            other => {
                let names: Vec<_> = FIELDS.iter().map(|(n, _)| *n).collect();
                return Err(format!(
                    "unknown setting `{other}`; expected one of: {}",
                    names.join(", ")
                ));
            }
        }
        Ok(())
    }

    /// `(name, display value)` pairs with keys masked.
    pub fn display_rows(&self) -> Vec<(&'static str, String)> {
        let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
        vec![
            ("backend", self.backend.to_string()),
            ("effort", show(&self.effort)),
            ("anthropic-key", mask(self.anthropic_api_key.as_deref())),
            ("openai-key", mask(self.openai_api_key.as_deref())),
            ("gemini-key", mask(self.gemini_api_key.as_deref())),
            ("claude-cli-model", show(&self.claude_cli_model)),
            ("anthropic-model", show(&self.anthropic_model)),
            ("openai-model", show(&self.openai_model)),
            ("gemini-model", show(&self.gemini_model)),
        ]
    }
}

/// Shows the first 5 and last 4 characters of a key.
pub fn mask(key: Option<&str>) -> String {
    match key {
        None => "-".into(),
        Some(k) if k.chars().count() <= 8 => "••••".into(),
        Some(k) => {
            let head: String = k.chars().take(5).collect();
            let tail: String = k
                .chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("{head}…{tail}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_private_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load_from(&path).unwrap(), Settings::default());

        let mut s = Settings::default();
        s.set("backend", "openai").unwrap();
        s.set("openai-key", "test-key-1234567890abcd").unwrap();
        s.set("gemini-model", "gemini-3.8-flash").unwrap();
        s.save_to(&path).unwrap();

        let loaded = Settings::load_from(&path).unwrap();
        assert_eq!(loaded, s);
        assert_eq!(loaded.backend, BackendChoice::OpenAi);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn set_validates_and_clears() {
        let mut s = Settings::default();
        assert!(s.set("backend", "llama").is_err());
        assert!(s.set("nope", "x").is_err());
        s.set("anthropic-key", "abc").unwrap();
        s.set("anthropic-key", "  ").unwrap();
        assert_eq!(s.anthropic_api_key, None);
    }

    #[test]
    fn masks_keys() {
        assert_eq!(mask(None), "-");
        assert_eq!(mask(Some("short")), "••••");
        assert_eq!(mask(Some("test-key-abcdefWXYZ")), "test-…WXYZ");
    }
}
