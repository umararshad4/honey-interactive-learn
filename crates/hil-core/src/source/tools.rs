//! Thin wrappers around the external programs we rely on.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

/// Finds an executable on `PATH`.
pub fn find_program(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let names = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            name.to_string(),
        ]
    } else {
        vec![name.to_string()]
    };
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|candidate| candidate.is_file())
}

/// Runs a command and returns stdout, or an error carrying the tail of stderr.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("`{program}` is not installed or not on PATH (run `hil doctor`)")
        } else {
            anyhow::Error::new(e).context(format!("failed to start `{program}`"))
        }
    })?;
    if !output.status.success() {
        bail!(
            "`{program}` failed ({}): {}",
            output.status,
            stderr_tail(&output.stderr)
        );
    }
    Ok(output)
}

pub fn stdout_string(output: &Output) -> Result<String> {
    String::from_utf8(output.stdout.clone()).context("program printed invalid UTF-8")
}

fn stderr_tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("WARNING"))
        .collect();
    let tail = &lines[lines.len().saturating_sub(4)..];
    if tail.is_empty() {
        "no error output".into()
    } else {
        tail.join("\n")
    }
}

/// Transcribes an audio or video file with the local `whisper` CLI.
pub fn whisper_transcribe(media: &Path, out_dir: &Path) -> Result<String> {
    let model = std::env::var("HIL_WHISPER_MODEL").unwrap_or_else(|_| "base".into());
    let media_str = media.to_string_lossy();
    let out_str = out_dir.to_string_lossy();
    run(
        "whisper",
        &[
            &media_str,
            "--model",
            &model,
            "--output_format",
            "txt",
            "--output_dir",
            &out_str,
            "--fp16",
            "False",
            "--verbose",
            "False",
        ],
        None,
    )?;
    let stem = media
        .file_stem()
        .context("media file has no name")?
        .to_string_lossy();
    let txt = out_dir.join(format!("{stem}.txt"));
    let text = std::fs::read_to_string(&txt)
        .with_context(|| format!("whisper did not write {}", txt.display()))?;
    Ok(text.trim().to_string())
}

/// Downloads the best audio stream of `url` into `dir` and returns its path.
pub fn download_audio(url: &str, dir: &Path) -> Result<PathBuf> {
    let template = dir.join("audio.%(ext)s");
    run(
        "yt-dlp",
        &[
            "--no-playlist",
            "--no-warnings",
            "--quiet",
            "-f",
            "bestaudio/best",
            "-o",
            &template.to_string_lossy(),
            url,
        ],
        None,
    )?;
    std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            p.file_stem().is_some_and(|s| s == "audio")
                && p.extension().is_some_and(|e| e != "part")
        })
        .context("yt-dlp finished but no audio file was written")
}
