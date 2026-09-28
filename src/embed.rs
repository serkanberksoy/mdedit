//! Embedded notes (E-01 / E-02): loading another note's lines (cached until
//! the file changes) and cutting out a heading's section.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use crate::blocks::analyze_cached;

/// A cached file: its modification time and size when read, and its lines.
type Entry = (Option<SystemTime>, u64, Arc<Vec<String>>);

/// The lines of the note at `path`, or `None` if it can't be read as text.
/// Embeds are drawn every frame, so files are cached and only read again
/// when their modification time or size changes.
pub fn load(path: &Path) -> Option<Arc<Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    let meta = std::fs::metadata(path).ok()?;
    let (modified, size) = (meta.modified().ok(), meta.len());
    let cache = CACHE.get_or_init(Default::default);
    if let Some((m, s, lines)) = cache.lock().ok()?.get(path)
        && (*m, *s) == (modified, size)
    {
        return Some(Arc::clone(lines));
    }
    let text = std::fs::read_to_string(path).ok()?;
    let lines = Arc::new(text.lines().map(String::from).collect::<Vec<_>>());
    cache
        .lock()
        .ok()?
        .insert(path.to_path_buf(), (modified, size, Arc::clone(&lines)));
    Some(lines)
}

/// The lines under heading `heading` (matched like links, see
/// [`crate::links::heading_matches`]): the heading itself up to the next
/// heading of the same or a higher level. `None` if there's no such heading.
pub fn section(lines: &[String], heading: &str) -> Option<Vec<String>> {
    let structure = analyze_cached(lines);
    let level_of = |i| structure.heading(lines, i);
    let start = (0..lines.len()).find(|&i| {
        level_of(i).is_some_and(|(_, text)| crate::links::heading_matches(text, heading))
    })?;
    let (level, _) = level_of(start).expect("found a heading");
    let end = (start + 1..lines.len())
        .find(|&i| level_of(i).is_some_and(|(lv, _)| lv <= level))
        .unwrap_or(lines.len());
    Some(lines[start..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn section_runs_to_the_next_heading_of_the_same_level() {
        let d = doc("# A\nintro\n## B\nb text\n### B1\ndeep\n## C\nc text");
        assert_eq!(section(&d, "B").unwrap(), doc("## B\nb text\n### B1\ndeep"));
        assert_eq!(section(&d, "c").unwrap(), doc("## C\nc text"));
        assert_eq!(
            section(&d, "A").unwrap().len(),
            8,
            "a level-1 section runs to the end"
        );
        assert!(section(&d, "nope").is_none());
    }

    #[test]
    fn code_is_not_a_heading_and_setext_headings_count() {
        let d = doc("## B\n```sh\n# a comment\n```\nafter\nSetext\n------\nbody");
        assert_eq!(
            section(&d, "B").unwrap(),
            doc("## B\n```sh\n# a comment\n```\nafter"),
            "the shell comment neither starts nor ends a section"
        );
        assert!(section(&d, "a comment").is_none());
        assert_eq!(section(&d, "Setext").unwrap(), doc("Setext\n------\nbody"));
    }
}
