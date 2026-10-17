# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-10-18

### Added
- `hil` CLI: `learn`, `restyle`, `list`, `show`, `quiz`, `cards`, `export`, `transcript`, `rm`, `doctor`.
- `hil-desktop`: GPUI app with a library sidebar, lesson tabs, a clickable quiz, flashcards and Markdown export.
- YouTube extraction through `yt-dlp` captions (manual first, then auto captions), with a local Whisper fallback.
- X / Twitter extraction through the public FxTwitter API: threads, quotes, long-form articles and Whisper for video posts.
- Lesson styles `plain`, `dev`, `visual` and `deep`, and depths `brief`, `compact` and `comprehensive`.
- Each lesson includes a TL;DR, key points, sections, an ASCII + Mermaid diagram, code snippets, real-world examples, flashcards, an MCQ quiz, open questions and a glossary.
- Two LLM backends: the `claude` CLI (reuses your Claude Code login) and the Anthropic Messages API (structured outputs, streaming, server-side refusal fallback).
- A local JSON library shared by the CLI and the desktop app, with quiz score history.
- CI (fmt, clippy, tests, MSRV, desktop build) and tag-driven release builds.

[Unreleased]: https://github.com/umararshad4/honey-interactive-learn/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/umararshad4/honey-interactive-learn/releases/tag/v0.1.0
