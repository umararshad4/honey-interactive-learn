//! Lessons saved as JSON files, shared by the CLI and the desktop app.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::model::{Lesson, Source};

pub struct Library {
    dir: PathBuf,
}

impl Library {
    /// `<data dir>/lessons` (see [`crate::settings::data_dir`]).
    pub fn open_default() -> Result<Self> {
        Self::open(crate::settings::data_dir()?)
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let dir = root.into().join("lessons");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("could not create {}", dir.display()))?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    pub fn save(&self, lesson: &Lesson) -> Result<()> {
        let path = self.path(&lesson.id);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(lesson)?)?;
        std::fs::rename(&tmp, &path).with_context(|| format!("could not save {}", path.display()))
    }

    /// All lessons, newest first. Unreadable files are skipped.
    pub fn list(&self) -> Result<Vec<Lesson>> {
        let mut lessons: Vec<Lesson> = std::fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .filter_map(|p| std::fs::read(&p).ok())
            .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
            .collect();
        lessons.sort_by_key(|l| std::cmp::Reverse(l.created_at));
        Ok(lessons)
    }

    pub fn load(&self, id: &str) -> Result<Lesson> {
        let bytes = std::fs::read(self.path(id)).with_context(|| format!("no lesson `{id}`"))?;
        serde_json::from_slice(&bytes).with_context(|| format!("lesson `{id}` is corrupted"))
    }

    /// Finds a lesson by list number (`1` = newest) or by id prefix.
    pub fn resolve(&self, query: &str) -> Result<Lesson> {
        let lessons = self.list()?;
        if lessons.is_empty() {
            bail!("your library is empty; create a lesson with `hil learn <url>`");
        }
        if let Ok(n) = query.parse::<usize>()
            && (1..=lessons.len()).contains(&n)
        {
            return Ok(lessons[n - 1].clone());
        }
        let matches: Vec<&Lesson> = lessons.iter().filter(|l| l.id.starts_with(query)).collect();
        match matches.as_slice() {
            [one] => Ok((*one).clone()),
            [] => bail!("no lesson matches `{query}` (see `hil list`)"),
            many => bail!(
                "`{query}` matches {} lessons; use more of the id: {}",
                many.len(),
                many.iter()
                    .map(|l| l.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        std::fs::remove_file(self.path(id)).with_context(|| format!("no lesson `{id}`"))
    }

    /// Reuses already-extracted text for a URL, skipping a re-download.
    pub fn find_source(&self, url: &str) -> Option<Source> {
        self.list()
            .ok()?
            .into_iter()
            .find(|l| l.source.url == url)
            .map(|l| l.source)
    }
}

/// Readable, unique-enough id: `rust-ownership-explained-3fa2`.
pub fn make_id(title: &str, seed: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    // Cut long titles at a word boundary.
    if slug.len() > 40 {
        let cut = slug[..40].rfind('-').unwrap_or(40);
        slug.truncate(cut);
    }
    let slug = slug.trim_end_matches('-');
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    let suffix = format!("{:04x}", h.finish() & 0xffff);
    if slug.is_empty() {
        format!("lesson-{suffix}")
    } else {
        format!("{slug}-{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use chrono::{Duration, Utc};

    fn lesson(id: &str, minutes_ago: i64) -> Lesson {
        Lesson {
            id: id.into(),
            created_at: Utc::now() - Duration::minutes(minutes_ago),
            style: Style::Plain,
            depth: Depth::Brief,
            generator: "test".into(),
            source: Source {
                url: format!("https://x.com/i/status/{minutes_ago}"),
                kind: SourceKind::X,
                title: "t".into(),
                author: "a".into(),
                duration_secs: None,
                origin: TranscriptOrigin::PostText,
                text: "text".into(),
                truncated: false,
            },
            content: LessonContent::default(),
            attempts: vec![],
        }
    }

    #[test]
    fn save_list_resolve_delete_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(tmp.path()).unwrap();
        lib.save(&lesson("rust-aaaa", 10)).unwrap();
        lib.save(&lesson("rust-bbbb", 1)).unwrap();
        lib.save(&lesson("go-cccc", 5)).unwrap();
        std::fs::write(lib.dir().join("junk.json"), "not json").unwrap();

        let ids: Vec<_> = lib.list().unwrap().into_iter().map(|l| l.id).collect();
        assert_eq!(ids, ["rust-bbbb", "go-cccc", "rust-aaaa"]);

        assert_eq!(lib.resolve("1").unwrap().id, "rust-bbbb");
        assert_eq!(lib.resolve("go").unwrap().id, "go-cccc");
        assert!(
            lib.resolve("rust")
                .unwrap_err()
                .to_string()
                .contains("matches 2")
        );
        assert!(lib.resolve("zzz").is_err());

        assert_eq!(
            lib.find_source("https://x.com/i/status/5").unwrap().url,
            "https://x.com/i/status/5"
        );

        lib.delete("go-cccc").unwrap();
        assert_eq!(lib.list().unwrap().len(), 2);
        assert!(lib.delete("go-cccc").is_err());
    }

    #[test]
    fn ids_are_slugs_with_suffix() {
        let id = make_id("Rust Ownership: Explained!", "seed");
        assert!(id.starts_with("rust-ownership-explained-"), "{id}");
        assert_eq!(id.len(), "rust-ownership-explained-".len() + 4);
        assert!(make_id("¿¿??", "s").starts_with("lesson-"));
        let long = make_id(
            "Rust Essentials: Memory Safety Through Ownership and Borrowing",
            "s",
        );
        assert!(
            long.starts_with("rust-essentials-memory-safety-through-"),
            "{long}"
        );
        assert!(!long.contains("-ow-"), "{long}");
    }
}
