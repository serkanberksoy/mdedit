//! Links under the cursor and how to follow them (F-05 / K-12). No vault:
//! a target is resolved relative to the current file's folder.

use std::path::{Path, PathBuf};

/// Where a link points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// A note or file: `path` as written (empty for `[[#Heading]]`, the
    /// current file), plus an optional `#Heading`.
    File {
        path: String,
        heading: Option<String>,
    },
    /// A web address (`https://…`, `mailto:` …); mdedit doesn't open these.
    Web(String),
}

/// The link at char column `col` of `line`, if the cursor is on one: a
/// wiki link `[[Note|alias]]` / `[[Note#Heading]]`, or a Markdown link
/// `[text](Note%20name.md)`.
pub fn link_at(line: &str, col: usize) -> Option<Link> {
    let at = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
    let covers = |start: usize, end: usize| start <= at && at < end;

    // Wiki links: [[target|alias]].
    let mut from = 0;
    while let Some(open) = line[from..].find("[[").map(|i| from + i) {
        let Some(close) = line[open + 2..].find("]]").map(|i| open + 2 + i) else {
            break;
        };
        if covers(open, close + 2) {
            let inner = &line[open + 2..close];
            let target = inner.split('|').next().unwrap_or_default();
            return Some(file_link(target.trim_end_matches('\\')));
        }
        from = close + 2;
    }

    // Markdown links: [text](url).
    let mut from = 0;
    while let Some(mid) = line[from..].find("](").map(|i| from + i) {
        let open = line[..mid].rfind('[');
        let close = line[mid + 2..].find(')').map(|i| mid + 2 + i);
        if let (Some(open), Some(close)) = (open, close)
            && covers(open, close + 1)
        {
            return Some(target_link(&line[mid + 2..close]));
        }
        from = mid + 2;
    }

    // Bare web addresses.
    for scheme in ["https://", "http://"] {
        let mut from = 0;
        while let Some(start) = line[from..].find(scheme).map(|i| from + i) {
            let end = line[start..]
                .find(char::is_whitespace)
                .map_or(line.len(), |i| start + i);
            if covers(start, end) {
                return Some(Link::Web(line[start..end].to_string()));
            }
            from = end;
        }
    }
    None
}

/// Every link in `line` (not embeds or images), with the text it shows
/// when rendered: `[[Note|alias]]` shows `alias`, `[text](url)` shows
/// `text`, a bare address itself. In the order they appear.
pub fn all_links(line: &str) -> Vec<(Link, String)> {
    let mut found: Vec<(usize, Link, String)> = Vec::new();
    let mut from = 0;
    while let Some(open) = line[from..].find("[[").map(|i| from + i) {
        let Some(close) = line[open + 2..].find("]]").map(|i| open + 2 + i) else {
            break;
        };
        if !line[..open].ends_with('!') {
            let inner = &line[open + 2..close];
            let target = inner.split('|').next().unwrap_or_default();
            let shown = crate::markdown::inline::link_text(inner);
            found.push((open, file_link(target.trim_end_matches('\\')), shown));
        }
        from = close + 2;
    }
    let mut from = 0;
    while let Some(mid) = line[from..].find("](").map(|i| from + i) {
        let open = line[..mid].rfind('[');
        let close = line[mid + 2..].find(')').map(|i| mid + 2 + i);
        if let (Some(open), Some(close)) = (open, close)
            && !line[..open].ends_with('!')
            && !line[open..mid].starts_with("[[")
        {
            let text = line[open + 1..mid].to_string();
            found.push((open, target_link(&line[mid + 2..close]), text));
        }
        from = mid + 2;
    }
    for scheme in ["https://", "http://"] {
        let mut from = 0;
        while let Some(start) = line[from..].find(scheme).map(|i| from + i) {
            let end = line[start..]
                .find(|c: char| c.is_whitespace() || c == ')')
                .map_or(line.len(), |i| start + i);
            // Not the target of a [text](url) link.
            if !line[..start].ends_with("](") {
                let url = line[start..end].to_string();
                found.push((start, Link::Web(url.clone()), url));
            }
            from = end;
        }
    }
    found.sort_by_key(|(at, _, _)| *at);
    found
        .into_iter()
        .map(|(_, link, text)| (link, text))
        .collect()
}

/// A file link from `target` (`Note#Heading`, or a block `Note#^id`, whose
/// `heading` starts with `^`).
fn file_link(target: &str) -> Link {
    let (path, fragment) = target.split_once('#').unwrap_or((target, ""));
    let heading = (!fragment.is_empty()).then(|| fragment.trim().to_string());
    Link::File {
        path: path.trim().to_string(),
        heading,
    }
}

/// Decodes `%20`-style escapes in a Markdown link target.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The note an embed line points to (E-01 / E-02): a line that is just
/// `![[Note]]` or `![[Note#Heading]]` (surrounding spaces allowed). Embeds
/// of other files (images, PDFs …) aren't notes and give `None`.
pub fn embed_target(line: &str) -> Option<Link> {
    let link = any_embed_target(line)?;
    let Link::File { path, .. } = &link else {
        return None;
    };
    is_note_path(path).then_some(link)
}

/// Whether a link target is a note (`.md`, `.markdown` or no extension).
pub fn is_note_path(path: &str) -> bool {
    match Path::new(path).extension() {
        None => true,
        Some(ext) => ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"),
    }
}

/// A line that is only an embed (`![[target]]`) of any file, a note or
/// not (a host may show other files: [`crate::resolver::Resolver::embed`]).
pub fn any_embed_target(line: &str) -> Option<Link> {
    let inner = line.trim().strip_prefix("![[")?.strip_suffix("]]")?;
    if inner.contains("]]") {
        return None;
    }
    Some(file_link(inner.split('|').next().unwrap_or_default()))
}

/// Splits a command-line argument like `note.md#My Title` into the file and
/// the heading to open at (F-06). A file whose name really contains `#` is
/// kept whole.
pub fn split_section(arg: &str) -> (PathBuf, Option<String>) {
    if !Path::new(arg).exists()
        && let Some((file, heading)) = arg.rsplit_once('#')
        && !file.is_empty()
    {
        return (PathBuf::from(file), Some(heading.to_string()));
    }
    (PathBuf::from(arg), None)
}

/// Whether heading text `heading` is what `wanted` asks for: the same text
/// ignoring case, or the same letters and digits (`#my-title` and
/// `#mytitle` both find "My Title").
pub fn heading_matches(heading: &str, wanted: &str) -> bool {
    let letters = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let wanted_letters = letters(wanted);
    !wanted_letters.is_empty()
        && (heading.trim().eq_ignore_ascii_case(wanted.trim())
            || letters(heading) == wanted_letters)
}

/// The file a link path points to, relative to `base` (the current file's
/// folder): the path as written if that file exists, else with `.md` added.
/// `Err` has the path that was looked for.
pub fn resolve(base: &Path, path: &str) -> Result<PathBuf, PathBuf> {
    let as_written = base.join(path);
    if as_written.is_file() {
        return Ok(as_written);
    }
    let mut with_md = as_written.into_os_string();
    with_md.push(".md");
    let with_md = PathBuf::from(with_md);
    if with_md.is_file() {
        Ok(with_md)
    } else {
        Err(with_md)
    }
}

/// A link reference definition (K-07): `[label]: target "title"`, up to
/// 3 spaces indented. Returns the label, the target (as written, `<…>`
/// included) and the rest of the line (the title).
pub fn link_definition(line: &str) -> Option<(&str, &str, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line[indent..].strip_prefix('[')?;
    let close = rest.find(']')?;
    let label = &rest[..close];
    // `[^label]:` is a footnote (X-05), not a link.
    if label.trim().is_empty() || label.contains('[') || label.starts_with('^') {
        return None;
    }
    let after = rest[close + 1..].strip_prefix(':')?;
    let value = after.trim_start();
    let target_len = if value.starts_with('<') {
        value.find('>')? + 1
    } else {
        value.find(char::is_whitespace).unwrap_or(value.len())
    };
    if target_len == 0 {
        return None;
    }
    Some((label, &value[..target_len], &value[target_len..]))
}

/// Where a Markdown link target points: `url`, `<path with spaces>`, a
/// `%20`-encoded path, with an optional title after it.
pub fn target_link(target: &str) -> Link {
    let target = target.trim();
    let target = match target.strip_prefix('<') {
        Some(rest) => rest.split('>').next().unwrap_or(rest),
        None => target.split_whitespace().next().unwrap_or(target),
    };
    if target.contains("://") || target.starts_with("mailto:") {
        return Link::Web(target.to_string());
    }
    file_link(&percent_decode(target))
}

/// The label of the reference link (`[text][label]`, or `[text][]` for
/// `text`) at char column `col` of `line`.
pub fn reference_at(line: &str, col: usize) -> Option<String> {
    let at = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
    let mut from = 0;
    while let Some(mid) = line[from..].find("][").map(|i| from + i) {
        let open = line[..mid].rfind('[');
        let close = line[mid + 2..].find(']').map(|i| mid + 2 + i);
        if let (Some(open), Some(close)) = (open, close)
            && !line[..open].ends_with('[')
            && (open..=close).contains(&at)
        {
            let label = &line[mid + 2..close];
            let label = if label.is_empty() {
                &line[open + 1..mid]
            } else {
                label
            };
            return Some(label.to_string());
        }
        from = mid + 2;
    }
    None
}

/// Where the reference `label` points, from its definition in `lines`
/// (labels match ignoring case).
pub fn find_definition(lines: &[String], label: &str) -> Option<Link> {
    lines
        .iter()
        .filter_map(|line| link_definition(line))
        .find(|(name, _, _)| name.trim().eq_ignore_ascii_case(label.trim()))
        .map(|(_, target, _)| target_link(target))
}

#[cfg(test)]
mod tests {

    #[test]
    fn link_definitions() {
        assert_eq!(
            link_definition("[docs]: https://example.com/docs"),
            Some(("docs", "https://example.com/docs", ""))
        );
        assert_eq!(
            link_definition("   [a b]:   <My Note.md> \"Title\""),
            Some(("a b", "<My Note.md>", " \"Title\""))
        );
        for line in [
            "[^1]: a footnote",
            "[docs]:",
            "[docs] : x",
            "    [docs]: x",
            "- [docs]: x",
            "[]: x",
            "text",
        ] {
            assert_eq!(link_definition(line), None, "{line}");
        }
    }

    #[test]
    fn reference_under_the_cursor() {
        let line = "See [the docs][Docs] and [short][] here";
        assert_eq!(reference_at(line, 5).as_deref(), Some("Docs"));
        assert_eq!(reference_at(line, 18).as_deref(), Some("Docs"));
        assert_eq!(reference_at(line, 27).as_deref(), Some("short"));
        assert_eq!(reference_at(line, 1), None);
        assert_eq!(reference_at("[[wiki]]", 3), None);
    }

    #[test]
    fn definitions_are_found_ignoring_case() {
        let lines: Vec<String> = [
            "text",
            "[docs]: https://example.com",
            "[note]: <My Note.md#Top> \"t\"",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            find_definition(&lines, "DOCS"),
            Some(Link::Web("https://example.com".into()))
        );
        assert_eq!(
            find_definition(&lines, "note"),
            Some(Link::File {
                path: "My Note.md".into(),
                heading: Some("Top".into())
            })
        );
        assert_eq!(find_definition(&lines, "other"), None);
    }

    #[test]
    fn a_markdown_link_title_is_not_part_of_the_target() {
        assert_eq!(
            link_at("[t](Note.md \"A title\")", 1),
            Some(Link::File {
                path: "Note.md".into(),
                heading: None
            })
        );
        assert_eq!(
            link_at("[t](<My Note.md>)", 1),
            Some(Link::File {
                path: "My Note.md".into(),
                heading: None
            })
        );
    }
    use super::*;

    fn file(path: &str, heading: Option<&str>) -> Option<Link> {
        Some(Link::File {
            path: path.into(),
            heading: heading.map(String::from),
        })
    }

    #[test]
    fn wiki_links() {
        let line = "see [[Note]] and [[Folder/Other|alias]] end";
        assert_eq!(link_at(line, 4), file("Note", None), "on the brackets");
        assert_eq!(link_at(line, 8), file("Note", None));
        assert_eq!(link_at(line, 11), file("Note", None), "on the closing ]]");
        assert_eq!(
            link_at(line, 30),
            file("Folder/Other", None),
            "on the alias"
        );
        assert_eq!(link_at(line, 2), None);
        assert_eq!(link_at(line, 13), None, "between links");
    }

    #[test]
    fn wiki_link_headings_and_table_escapes() {
        assert_eq!(link_at("[[Note#Two]]", 3), file("Note", Some("Two")));
        assert_eq!(link_at("[[#Local]]", 3), file("", Some("Local")));
        assert_eq!(
            link_at("[[Note#^block1]]", 3),
            file("Note", Some("^block1")),
            "a block ref keeps its id (E-03)"
        );
        assert_eq!(link_at(r"| [[Note\|alias]] |", 5), file("Note", None));
    }

    #[test]
    fn markdown_links() {
        let line = "a [text](sub/My%20Note.md#Part) b";
        assert_eq!(link_at(line, 3), file("sub/My Note.md", Some("Part")));
        assert_eq!(link_at(line, 12), file("sub/My Note.md", Some("Part")));
        assert_eq!(link_at(line, 0), None);
    }

    #[test]
    fn web_links_are_recognised_but_not_files() {
        assert_eq!(
            link_at("[site](https://example.com)", 2),
            Some(Link::Web("https://example.com".into()))
        );
        assert_eq!(
            link_at("go https://example.com now", 5),
            Some(Link::Web("https://example.com".into()))
        );
    }

    #[test]
    fn columns_are_chars_not_bytes() {
        assert_eq!(link_at("é [[Nöte]]", 4), file("Nöte", None));
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("mdedit-link-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        d
    }

    #[test]
    fn embed_lines() {
        assert_eq!(embed_target("![[Note]]"), file("Note", None));
        assert_eq!(
            embed_target("  ![[sub/Note#Part]]  "),
            file("sub/Note", Some("Part"))
        );
        assert_eq!(embed_target("![[Note.md|shown]]"), file("Note.md", None));
        assert_eq!(embed_target("![[picture.png]]"), None, "not a note");
        assert_eq!(
            embed_target("see ![[Note]] inline"),
            None,
            "only whole lines"
        );
        assert_eq!(embed_target("[[Note]]"), None, "a link, not an embed");
    }

    #[test]
    fn command_line_section_is_split_off() {
        let d = scratch("split");
        let p = d.join("note.md");
        std::fs::write(&p, "").unwrap();
        let arg = format!("{}#My Title", p.display());
        assert_eq!(split_section(&arg), (p.clone(), Some("My Title".into())));
        assert_eq!(split_section(&p.display().to_string()), (p.clone(), None));
        let hashed = d.join("c#.md");
        std::fs::write(&hashed, "").unwrap();
        assert_eq!(split_section(&hashed.display().to_string()), (hashed, None));
        assert_eq!(
            split_section("new.md#Top"),
            (PathBuf::from("new.md"), Some("Top".into()))
        );
    }

    #[test]
    fn headings_match_ignoring_case_spaces_and_punctuation() {
        assert!(heading_matches("My Title", "my title"));
        assert!(heading_matches("My Title", "my-title"));
        assert!(heading_matches("My Title", "mytitle"));
        assert!(heading_matches("Week 1: ML", "week-1-ml"));
        assert!(!heading_matches("My Title", "other"));
        assert!(!heading_matches("My Title", ""));
    }

    #[test]
    fn resolve_adds_md_when_needed() {
        let d = scratch("resolve");
        std::fs::write(d.join("Note.md"), "").unwrap();
        std::fs::write(d.join("sub").join("c.md"), "").unwrap();
        std::fs::write(d.join("v1.2.md"), "").unwrap();
        assert_eq!(resolve(&d, "Note"), Ok(d.join("Note.md")));
        assert_eq!(resolve(&d, "Note.md"), Ok(d.join("Note.md")));
        assert_eq!(resolve(&d, "sub/c"), Ok(d.join("sub").join("c.md")));
        assert_eq!(resolve(&d, "v1.2"), Ok(d.join("v1.2.md")), "dots in names");
        assert_eq!(resolve(&d, "missing"), Err(d.join("missing.md")));
    }
}
