//! Embedded notes (E-01 … E-03): loading another note's lines (cached until
//! the file changes) and cutting out a heading's section or a block.

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

/// A fragment's lines: a block for `^id` ([`block`]), else a heading's
/// [`section`].
pub fn part(lines: &[String], fragment: &str) -> Option<Vec<String>> {
    match fragment.strip_prefix('^') {
        Some(id) => block(lines, id),
        None => section(lines, fragment),
    }
}

/// Whether `line` ends with the block id `^id` (or is only it).
fn has_id(line: &str, id: &str) -> bool {
    let t = line.trim_end();
    let Some(before) = t.strip_suffix(id).and_then(|b| b.strip_suffix('^')) else {
        return false;
    };
    before.is_empty() || before.ends_with(char::is_whitespace)
}

/// The line with block id `id` (E-03), if any.
pub fn block_line(lines: &[String], id: &str) -> Option<usize> {
    (0..lines.len()).find(|&i| has_id(&lines[i], id))
}

/// Block `^id` (E-03): the paragraph or list item (with its sub-items)
/// the id ends, without the id; an id on a line of its own marks the block
/// just above it (a table, a quote, a list). `None` if there's no such id.
pub fn block(lines: &[String], id: &str) -> Option<Vec<String>> {
    let at = block_line(lines, id)?;
    let blank = |i: usize| lines[i].trim().is_empty();
    let strip = |line: &str| {
        let t = line.trim_end();
        t[..t.len() - id.len() - 1].trim_end().to_string()
    };
    if strip(&lines[at]).trim().is_empty() {
        // The block above, past blank lines.
        let end = (0..at).rev().find(|&i| !blank(i))?;
        let start = (0..end)
            .rev()
            .take_while(|&i| !blank(i))
            .last()
            .unwrap_or(end);
        return Some(lines[start..=end].to_vec());
    }
    let item = |line: &str| {
        let t = line.trim_start();
        t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") || {
            let digits = t.chars().take_while(char::is_ascii_digit).count();
            digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") "))
        }
    };
    let indent = |line: &str| line.len() - line.trim_start().len();
    let mut out = Vec::new();
    if item(&lines[at]) {
        // The item and the lines indented under it.
        out.push(strip(&lines[at]));
        let own = indent(&lines[at]);
        out.extend(
            lines[at + 1..]
                .iter()
                .take_while(|l| !l.trim().is_empty() && indent(l) > own)
                .cloned(),
        );
        // Written as the note's first level.
        return Some(
            out.iter()
                .map(|l| l[own.min(indent(l))..].to_string())
                .collect(),
        );
    }
    // A paragraph: back to the blank line (or heading, or item) above.
    let start = (0..at)
        .rev()
        .take_while(|&i| !blank(i) && !item(&lines[i]) && !lines[i].trim_start().starts_with('#'))
        .last()
        .unwrap_or(at);
    out.extend(lines[start..at].iter().cloned());
    out.push(strip(&lines[at]));
    Some(out)
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
    fn blocks_by_their_id() {
        let d = doc(
            "# T\nintro\n\nfirst\nsecond ^p1\n\n- a\n  - b ^i1\n    - c\n- d\n\n> quote\n> more\n\n^q\nx^notanid",
        );
        assert_eq!(block(&d, "p1").unwrap(), doc("first\nsecond"));
        assert_eq!(block(&d, "i1").unwrap(), doc("- b\n  - c"));
        assert_eq!(block(&d, "q").unwrap(), doc("> quote\n> more"));
        assert!(block(&d, "notanid").is_none(), "an id follows a space");
        assert!(block(&d, "zz").is_none());
        assert_eq!(block_line(&d, "i1"), Some(7));
        assert_eq!(part(&d, "^p1"), block(&d, "p1"));
        assert_eq!(part(&d, "T").unwrap().len(), d.len());
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
