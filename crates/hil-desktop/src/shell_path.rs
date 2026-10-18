//! Apps opened from Finder or the Dock start with a minimal environment:
//! PATH is `/usr/bin:/bin:/usr/sbin:/sbin` and nothing from `~/.zshrc` is set.
//! So `yt-dlp`, `whisper` and `claude` would not be found, and API keys
//! exported in the shell would be missing. Borrow them from the user's login
//! shell, plus the usual install locations.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MARKER: &str = "__HIL_ENV__";

/// Variables copied from the login shell when they are not already set.
const IMPORTED_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_BASE_URL",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GEMINI_BASE_URL",
    "HIL_HOME",
    "HIL_BACKEND",
    "HIL_MODEL",
    "HIL_EFFORT",
    "HIL_WHISPER_MODEL",
];

/// Fills in PATH and provider variables when the app was not started from a terminal.
/// Must run before any other thread starts (it calls `set_var`).
pub fn import_shell_env_for_gui_launch() {
    if std::env::var_os("TERM").is_some() {
        return;
    }
    let shell_env = login_shell_env().unwrap_or_default();

    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut extra: Vec<PathBuf> = shell_env
        .get("PATH")
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for dir in [".local/bin", ".cargo/bin", ".bun/bin", ".npm-global/bin"] {
            extra.push(home.join(dir));
        }
    }
    extra.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    let merged = merge_paths(std::env::split_paths(&current).collect(), extra);

    // SAFETY: called at startup before any threads are spawned.
    unsafe {
        if let Ok(joined) = std::env::join_paths(merged) {
            std::env::set_var("PATH", joined);
        }
        for name in IMPORTED_VARS {
            if std::env::var_os(name).is_none()
                && let Some(value) = shell_env.get(*name).filter(|v| !v.is_empty())
            {
                std::env::set_var(name, value);
            }
        }
    }
}

/// Keeps `base` order, appends new existing directories from `extra`.
fn merge_paths(mut base: Vec<PathBuf>, extra: Vec<PathBuf>) -> Vec<PathBuf> {
    for dir in extra {
        if !base.contains(&dir) && dir.is_dir() {
            base.push(dir);
        }
    }
    base
}

/// Runs `$SHELL -l -i -c env` and parses the output, giving up after a few seconds.
fn login_shell_env() -> Option<HashMap<String, String>> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut child = Command::new(shell)
        .args([
            "-l",
            "-i",
            "-c",
            &format!("printf '{MARKER}'; env; printf '{MARKER}'"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on a thread so a chatty shell cannot fill the pipe and stall.
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    let out = reader.join().ok()?;
    parse_env_block(&out)
}

/// Parses `KEY=value` lines between the markers, ignoring shell startup noise.
fn parse_env_block(output: &str) -> Option<HashMap<String, String>> {
    let start = output.find(MARKER)? + MARKER.len();
    let len = output[start..].find(MARKER)?;
    let vars = output[start..start + len]
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Some(vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_env_between_markers_despite_noise() {
        let out = "zoxide: warning\n__HIL_ENV__PATH=/opt/homebrew/bin:/usr/bin\nOPENAI_API_KEY=abc=def\n  continuation line\n__HIL_ENV__bye";
        let vars = parse_env_block(out).unwrap();
        assert_eq!(vars["PATH"], "/opt/homebrew/bin:/usr/bin");
        assert_eq!(vars["OPENAI_API_KEY"], "abc=def");
        assert_eq!(vars.len(), 2);
        assert!(parse_env_block("no markers").is_none());
    }

    #[test]
    fn merge_keeps_order_and_skips_duplicates_and_missing_dirs() {
        let tmp = std::env::temp_dir();
        let merged = merge_paths(
            vec![PathBuf::from("/usr/bin")],
            vec![
                PathBuf::from("/usr/bin"),
                tmp.clone(),
                PathBuf::from("/definitely/not/here"),
            ],
        );
        assert_eq!(merged, vec![PathBuf::from("/usr/bin"), tmp]);
    }
}
