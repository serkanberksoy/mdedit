//! Inline Markdown: emphasis and friends, code spans, escapes, wiki links,
//! embeds, URLs and tags, as styled spans.

use super::*;

/// Paired inline markers (emphasis and friends): the marker text and the
/// style its content gets. Longer markers of the same character come first.
const DELIMITERS: &[(&str, Style)] = &[
    (
        "***",
        Style::new().add_modifier(Modifier::BOLD.union(Modifier::ITALIC)),
    ),
    ("**", Style::new().add_modifier(Modifier::BOLD)),
    ("*", Style::new().add_modifier(Modifier::ITALIC)),
    (
        "___",
        Style::new().add_modifier(Modifier::BOLD.union(Modifier::ITALIC)),
    ),
    ("__", Style::new().add_modifier(Modifier::BOLD)),
    ("_", Style::new().add_modifier(Modifier::ITALIC)),
    ("~~", Style::new().add_modifier(Modifier::CROSSED_OUT)),
    // Highlight: black on yellow reads well in light and dark themes.
    ("==", Style::new().fg(Color::Black).bg(Color::Yellow)),
];

/// Highlight colors (T-07a): a highlight starting with one of these emoji
/// (`==🔴text==`) gets that background, and the emoji isn't shown:
/// (emoji, color, name). Named colors, so a palette can change them.
pub const HIGHLIGHT_COLORS: [(&str, Color, &str); 6] = [
    ("🔴", Color::Red, "red"),
    ("🟠", Color::LightRed, "orange"),
    ("🟡", Color::Yellow, "yellow"),
    ("🟢", Color::Green, "green"),
    ("🔵", Color::Blue, "blue"),
    ("🟣", Color::Magenta, "purple"),
];

/// A highlight's color emoji at the start of its `content`: its length
/// and color (there must be text after it).
fn highlight_color(content: &str) -> Option<(usize, Color)> {
    HIGHLIGHT_COLORS.iter().find_map(|&(emoji, color, _)| {
        let rest = content.strip_prefix(emoji)?;
        (!rest.is_empty()).then_some((emoji.len(), color))
    })
}

thread_local! {
    /// The host's verbatim spans ([`set_verbatim`]).
    static VERBATIM: std::cell::RefCell<std::rc::Rc<Vec<(String, String)>>> =
        std::cell::RefCell::default();
}

/// Text from `open` to `close` (each pair) is shown as written, markers
/// too, in the code color: nothing in it is Markdown. For a host whose
/// notes have their own syntax (template tags: `("<%", "%>")`). For the
/// thread that draws; empty (the default) turns it off.
pub fn set_verbatim(pairs: &[(&str, &str)]) {
    let pairs = pairs
        .iter()
        .filter(|(open, close)| !open.is_empty() && !close.is_empty())
        .map(|(open, close)| (open.to_string(), close.to_string()))
        .collect();
    VERBATIM.with(|v| *v.borrow_mut() = std::rc::Rc::new(pairs));
}

thread_local! {
    /// The `[[targets]]` of the line being drawn that point nowhere
    /// ([`with_missing`]).
    static MISSING: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs `f` (drawing a line) with wiki links to `missing` targets (as
/// written: `Note`, `Note#Heading`) dimmed (K-11).
pub fn with_missing<T>(missing: Vec<String>, f: impl FnOnce() -> T) -> T {
    let before = MISSING.with(|m| std::mem::replace(&mut *m.borrow_mut(), missing));
    let out = f();
    MISSING.with(|m| *m.borrow_mut() = before);
    out
}

/// A host's function from a link's target (or a span's text) to what's
/// shown ([`set_link_badge`], [`set_rendered`]).
pub type HostText = std::rc::Rc<dyn Fn(&str) -> Option<String>>;

thread_local! {
    /// The host's link badges ([`set_link_badge`]).
    static BADGE: std::cell::RefCell<Option<HostText>> = const { std::cell::RefCell::new(None) };
    /// The host's rendered spans ([`set_rendered`]).
    static RENDERED: std::cell::RefCell<std::rc::Rc<Vec<(String, String, HostText)>>> =
        std::cell::RefCell::default();
}

/// A short text the host shows dimmed after a wiki link (`3`: how many
/// notes link to it), from the link's target (`Note`, `Note#Heading`);
/// `None` shows nothing. For the thread that draws; `None` turns it off.
pub fn set_link_badge(badge: Option<HostText>) {
    BADGE.with(|b| *b.borrow_mut() = badge);
}

/// Spans the host shows its own way: from each `open` to its `close`
/// (`[@` … `]`: a citation), what `render` gives for the text between
/// (`Doe 2020`), in a link's color; `None` leaves the span as written.
/// For the thread that draws; empty turns it off.
pub fn set_rendered(spans: Vec<(String, String, HostText)>) {
    let spans = spans
        .into_iter()
        .filter(|(open, close, _)| !open.is_empty() && !close.is_empty())
        .collect();
    RENDERED.with(|r| *r.borrow_mut() = std::rc::Rc::new(spans));
}

/// What a rendered span `rest` starts with shows, and its length.
fn rendered(rest: &str, spans: &[(String, String, HostText)]) -> Option<(usize, String)> {
    spans.iter().find_map(|(open, close, render)| {
        let inner = rest.strip_prefix(open.as_str())?;
        let end = inner.find(close.as_str())?;
        let shown = render(&inner[..end])?;
        Some((open.len() + end + close.len(), shown))
    })
}

/// A host's table cells ([`set_table_cells`]).
pub type HostTable = std::rc::Rc<dyn Fn(&[String]) -> Option<Vec<String>>>;

thread_local! {
    /// The host's table cells ([`set_table_cells`]).
    static TABLES: std::cell::RefCell<Option<HostTable>> = const { std::cell::RefCell::new(None) };
}

/// A host's own table cells (computed ones: `=SUM(B2:B4)` shown as its
/// result): given a table's lines (its second the separator), the lines
/// to show instead, as many, or `None`. Shown while the cursor isn't in
/// the table; in it, the table is as written. For the thread that draws;
/// `None` turns it off.
pub fn set_table_cells(cells: Option<HostTable>) {
    TABLES.with(|t| *t.borrow_mut() = cells);
}

/// The lines the host shows for a table ([`set_table_cells`]).
pub(crate) fn host_table(lines: &[String]) -> Option<Vec<String>> {
    TABLES.with(|t| {
        let host = t.borrow().clone()?;
        host(lines).filter(|shown| shown.len() == lines.len())
    })
}

/// Which lines a host hides ([`set_hidden_lines`]).
pub type HostLines = std::rc::Rc<dyn Fn(&str) -> bool>;

thread_local! {
    /// The host's hidden lines ([`set_hidden_lines`]).
    static HIDDEN: std::cell::RefCell<Option<HostLines>> = const { std::cell::RefCell::new(None) };
}

/// Lines the host hides in the live preview and view mode (a table's
/// formula comment): they take no rows until the cursor is on them, when
/// they show as written. For the thread that draws; `None` turns it off.
pub fn set_hidden_lines(hide: Option<HostLines>) {
    HIDDEN.with(|h| *h.borrow_mut() = hide);
}

/// Whether the host hides `line` ([`set_hidden_lines`]).
pub(crate) fn host_hides(line: &str) -> bool {
    HIDDEN.with(|h| h.borrow().as_ref().is_some_and(|hide| hide(line)))
}

/// The length of the verbatim span `rest` starts with, if it does.
fn verbatim_len(rest: &str, pairs: &[(String, String)]) -> Option<usize> {
    pairs.iter().find_map(|(open, close)| {
        let inner = rest.strip_prefix(open.as_str())?;
        let end = inner.find(close.as_str())?;
        Some(open.len() + end + close.len())
    })
}

/// A host's marks: the byte ranges of a text to show in a style of their
/// own.
pub type HostMarks = std::rc::Rc<dyn Fn(&str) -> Vec<(std::ops::Range<usize>, Style)>>;

thread_local! {
    /// The host's marks ([`set_marks`]).
    static MARKS: std::cell::RefCell<Option<HostMarks>> = const { std::cell::RefCell::new(None) };
}

/// Parts of a text the host styles its own way (a task's dates as muted
/// chips): given a line's text (after its list marker or heading), the
/// byte ranges to show as written, nothing in them Markdown, with their
/// style patched on. A range inside other markup (a link, `**…**`) is left
/// as it was. For the thread that draws; `None` turns it off.
pub fn set_marks(marks: Option<HostMarks>) {
    MARKS.with(|m| *m.borrow_mut() = marks);
}

/// Parses inline markup in `text`, returning styled spans with the syntax
/// hidden. `base` is patched with each token's style; markup nests
/// (`~~a **b** c~~`).
pub fn inline_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut marks = MARKS
        .with(|m| m.borrow().clone())
        .map(|host| host(text))
        .unwrap_or_default();
    marks.retain(|(r, _)| {
        r.start < r.end && text.is_char_boundary(r.start) && text.is_char_boundary(r.end)
    });
    marks.sort_by_key(|(r, _)| r.start);
    let mut out = Vec::new();
    parse_inline(text, base, &marks, &mut out);
    out
}

fn parse_inline(
    text: &str,
    base: Style,
    marks: &[(std::ops::Range<usize>, Style)],
    out: &mut Vec<Span<'static>>,
) {
    let link = Style::default().fg(Color::LightBlue);
    let url = Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::UNDERLINED);
    let tag = Style::default()
        .fg(Color::LightCyan)
        .bg(Color::Rgb(25, 55, 100));

    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Span<'static>>| {
        if !plain.is_empty() {
            out.push(Span::styled(std::mem::take(plain), base));
        }
    };
    let verbatim = VERBATIM.with(|v| std::rc::Rc::clone(&v.borrow()));
    let spans = RENDERED.with(|r| std::rc::Rc::clone(&r.borrow()));
    let badge = BADGE.with(|b| b.borrow().clone());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let prev = text[..i].chars().next_back();

        // A host's mark: as written, in its style.
        if let Some((range, style)) = marks.iter().find(|(r, _)| r.start == i) {
            flush(&mut plain, out);
            out.push(Span::styled(
                text[range.clone()].to_string(),
                base.patch(*style),
            ));
            i = range.end;
            continue;
        }

        // A host's rendered span (`[@key]` as `Doe 2020`).
        if !spans.is_empty()
            && let Some((n, shown)) = rendered(rest, &spans)
        {
            flush(&mut plain, out);
            out.push(Span::styled(shown, base.patch(link)));
            i += n;
            continue;
        }

        // A host's verbatim span (`<% … %>`): as written.
        if !verbatim.is_empty()
            && let Some(n) = verbatim_len(rest, &verbatim)
        {
            flush(&mut plain, out);
            out.push(Span::styled(
                rest[..n].to_string(),
                base.patch(Style::new().fg(CODE)),
            ));
            i += n;
            continue;
        }

        // Backslash escape: `\*` is a literal `*` (ASCII punctuation only).
        if let Some(c) = rest
            .strip_prefix('\\')
            .and_then(|r| r.chars().next())
            .filter(char::is_ascii_punctuation)
        {
            plain.push(c);
            i += 1 + c.len_utf8();
            continue;
        }

        // Code span: a run of backticks up to the next run of the same length.
        // Nothing inside is parsed. An unmatched run is literal text.
        if rest.starts_with('`') {
            let n = run_len(text, i, '`');
            let open = i + n;
            let close = (open..text.len()).find(|&j| {
                text.is_char_boundary(j)
                    && text[j..].starts_with('`')
                    && !text[..j].ends_with('`')
                    && run_len(text, j, '`') == n
            });
            match close {
                Some(j) => {
                    flush(&mut plain, out);
                    let mut code = &text[open..j];
                    // One space on both sides is padding (`` `x` ``).
                    if code.len() > 2
                        && code.starts_with(' ')
                        && code.ends_with(' ')
                        && !code.trim().is_empty()
                    {
                        code = &code[1..code.len() - 1];
                    }
                    out.push(Span::styled(
                        code.to_string(),
                        base.patch(Style::new().fg(CODE)),
                    ));
                    i = j + n;
                }
                None => {
                    plain.push_str(&text[i..open]);
                    i = open;
                }
            }
            continue;
        }

        // Comment `%%…%%` (X-07): dimmed, the markers hidden.
        if let Some(inner) = rest.strip_prefix("%%")
            && let Some(end) = inner.find("%%")
        {
            flush(&mut plain, out);
            let dim = Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC);
            out.push(Span::styled(inner[..end].to_string(), base.patch(dim)));
            i += 2 + end + 2;
            continue;
        }

        // Footnotes (X-04 … X-06): a reference `[^label]` (a definition's
        // `[^label]:` at the line start) as `[label]`; an inline footnote
        // `^[text]` as `[text]`.
        let note = Style::new().fg(Color::Cyan);
        if let Some(inner) = rest.strip_prefix("[^")
            && let Some(end) = inner.find(']')
            && !inner[..end].is_empty()
            && !inner[..end].contains(char::is_whitespace)
        {
            flush(&mut plain, out);
            out.push(Span::styled(
                format!("[{}]", &inner[..end]),
                base.patch(note),
            ));
            i += 2 + end + 1;
            if i == 2 + end + 1 && text[i..].starts_with(':') {
                i += 1; // the definition's colon
            }
            continue;
        }
        if let Some(inner) = rest.strip_prefix("^[")
            && let Some(end) = inner.find(']')
        {
            flush(&mut plain, out);
            out.push(Span::styled(
                format!("[{}]", &inner[..end]),
                base.patch(note),
            ));
            i += 2 + end + 1;
            continue;
        }

        // Markdown link `[text](target)` (K-06): the text, parsed for
        // markup, in the link style; an inline image `![alt](…)` as `🖼 alt`.
        if !rest.starts_with("[[")
            && !rest.starts_with("![[")
            && let Some((len, label, target)) = markdown_link(rest)
        {
            flush(&mut plain, out);
            let web = ["http://", "https://", "mailto:"]
                .iter()
                .any(|p| target.starts_with(p));
            let style = base.patch(if web { url } else { link });
            if rest.starts_with('!') {
                out.push(Span::styled(format!("🖼 {label}"), base.patch(link)));
            } else {
                parse_inline(label, style, &[], out);
            }
            i += len;
            continue;
        }

        // Reference link `[text][ref]` / `[text][]` (K-07): the text.
        if !rest.starts_with("[[")
            && let Some((len, label)) = reference_link(rest)
        {
            flush(&mut plain, out);
            parse_inline(label, base.patch(link), &[], out);
            i += len;
            continue;
        }

        // Atomic tokens: shown with their own style, content not re-parsed.
        // A wiki link's badge from the host, after it.
        let mut after = None;
        let token: Option<(usize, String, Style)> = if rest.starts_with("![[") {
            // An embed shown inline (not expanded): a marker and the name.
            rest.find("]]").map(|end| {
                let shown = link_text(&rest[3..end]);
                (end + 2, format!("⧉ {shown}"), link)
            })
        } else if rest.starts_with("[[") {
            rest.find("]]").map(|end| {
                let inner = &rest[2..end];
                let target = inner.split('|').next().unwrap_or_default();
                let target = target.trim_end_matches('\\').trim();
                let missing = MISSING.with(|m| m.borrow().iter().any(|t| t == target));
                let style = if missing {
                    link.add_modifier(Modifier::DIM)
                } else {
                    link
                };
                after = badge.as_ref().and_then(|b| b(target));
                (end + 2, link_text(inner), style)
            })
        } else if let Some(address) = autolink(rest) {
            // `<https://…>` / `<me@example.com>` (K-08): without the brackets.
            Some((address.len() + 2, address.to_string(), url))
        } else if rest.starts_with("https://") || rest.starts_with("http://") {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            Some((end, rest[..end].to_string(), url))
        } else if let Some(id) = block_id(rest).filter(|_| prev.is_some_and(char::is_whitespace)) {
            // A block ID (K-05) ends the line: dimmed.
            Some((
                rest.len(),
                id.to_string(),
                Style::default().fg(Color::DarkGray),
            ))
        } else if rest.starts_with('#') && prev.is_none_or(char::is_whitespace) {
            let name = &rest[1..];
            let len = name
                .find(|c: char| !(c.is_alphanumeric() || "_-/".contains(c)))
                .unwrap_or(name.len());
            let name = &name[..len];
            (!name.is_empty() && !name.chars().all(|c| c.is_ascii_digit()))
                .then(|| (len + 1, format!("#{name}"), tag))
        } else {
            None
        };
        if let Some((len, shown, style)) = token {
            flush(&mut plain, out);
            out.push(Span::styled(shown, base.patch(style)));
            if let Some(b) = after {
                let dim = Style::new().fg(Color::DarkGray);
                out.push(Span::styled(format!(" {b}"), base.patch(dim)));
            }
            i += len;
            continue;
        }

        // Paired markers: the content is parsed again with the added style.
        if let Some((open, close, style)) = delimited(text, i) {
            flush(&mut plain, out);
            let marker = open - i;
            // A highlight's color emoji colors it, unseen (T-07a).
            let (start, style) = match highlight_color(&text[open..close]) {
                Some((len, color)) if rest.starts_with("==") => (open + len, style.bg(color)),
                _ => (open, style),
            };
            parse_inline(&text[start..close], base.patch(style), &[], out);
            i = close + marker;
            continue;
        }

        let c = rest.chars().next().expect("i is inside text");
        plain.push(c);
        i += c.len_utf8();
    }
    flush(&mut plain, out);
}

/// What a wiki link `[[inner]]` shows: its alias, or the note and heading
/// as `Note › Heading` (K-03, K-04); a heading in the same note
/// (`[[#Heading]]`) is just `Heading`.
pub(crate) fn link_text(inner: &str) -> String {
    if let Some((_, alias)) = inner.split_once('|') {
        return alias.to_string();
    }
    let inner = inner.trim_end_matches('\\');
    match inner.split_once('#') {
        Some(("", heading)) => heading.to_string(),
        Some((note, heading)) => format!("{note} › {heading}"),
        None => inner.to_string(),
    }
}

/// A block ID (`^abc-123`, K-05) if `rest` is one up to the end of the
/// line (trailing spaces allowed).
fn block_id(rest: &str) -> Option<&str> {
    let id = rest.trim_end();
    let name = id.strip_prefix('^')?;
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then_some(id)
}

/// A Markdown link or image at the start of `rest`: `[text](target)` or
/// `![alt](target)`, with an optional `<…>` around the target and a title
/// after it. Returns (length, text, target).
fn markdown_link(rest: &str) -> Option<(usize, &str, &str)> {
    let open = if rest.starts_with("![") { 2 } else { 1 };
    if !rest[open - 1..].starts_with('[') {
        return None;
    }
    let close = open + rest[open..].find(']')?;
    let label = &rest[open..close];
    let after = rest[close + 1..].strip_prefix('(')?;
    let (target, len) = match after.strip_prefix('<') {
        Some(bracketed) => {
            let end = bracketed.find('>')?;
            let tail = &bracketed[end + 1..];
            let paren = tail.find(')')?;
            (&bracketed[..end], 1 + end + 1 + paren + 1)
        }
        None => {
            let paren = after.find(')')?;
            let inside = after[..paren].trim();
            (inside.split_whitespace().next().unwrap_or(""), paren + 1)
        }
    };
    (!label.is_empty() && !target.is_empty()).then_some((close + 2 + len, label, target))
}

/// A reference link at the start of `rest`: `[text][ref]` or `[text][]`.
/// Returns (length, text).
fn reference_link(rest: &str) -> Option<(usize, &str)> {
    let inner = rest.strip_prefix('[')?;
    let close = inner.find(']')?;
    let label = &inner[..close];
    let after = inner[close + 1..].strip_prefix('[')?;
    let end = after.find(']')?;
    let reference = &after[..end];
    let ok = !label.is_empty() && !label.contains('[') && !reference.contains('[');
    ok.then_some((1 + close + 2 + end + 1, label))
}

/// The address of an autolink at the start of `rest` (K-08): `<scheme:…>`
/// for http(s) and mailto, or `<name@domain.tld>`; no spaces inside.
fn autolink(rest: &str) -> Option<&str> {
    let inner = rest.strip_prefix('<')?;
    let address = &inner[..inner.find('>')?];
    if address.is_empty() || address.contains(char::is_whitespace) || address.contains('<') {
        return None;
    }
    let scheme = ["https://", "http://", "mailto:"]
        .iter()
        .any(|s| address.starts_with(s));
    let email = address
        .split_once('@')
        .is_some_and(|(name, domain)| !name.is_empty() && domain.contains('.'));
    (scheme || email).then_some(address)
}

/// Length of the run of `c` starting at byte `at` of `text`.
fn run_len(text: &str, at: usize, c: char) -> usize {
    text[at..].chars().take_while(|&x| x == c).count()
}

/// If a paired marker opens at byte `i`, returns (content start, content
/// end, style). The opener must be followed by a non-space and the closer
/// preceded by one; the closer's run of marker chars must be exactly as
/// long as the marker. `_` markers can't be inside a word (`snake_case`).
fn delimited(text: &str, i: usize) -> Option<(usize, usize, Style)> {
    let first = text[i..].chars().next()?;
    let run = run_len(text, i, first);
    for &(marker, style) in DELIMITERS {
        let c = marker.chars().next().expect("markers aren't empty");
        if c != first || marker.len() > run {
            continue;
        }
        let open = i + marker.len();
        if text[open..].chars().next().is_none_or(char::is_whitespace) {
            continue;
        }
        let in_word = |ch: Option<char>| ch.is_some_and(char::is_alphanumeric);
        if c == '_' && in_word(text[..i].chars().next_back()) {
            continue;
        }
        let mut j = open + 1;
        while j < text.len() {
            if text.is_char_boundary(j)
                && text[j..].starts_with(marker)
                && run_len(text, j, c) == marker.len()
                && !text[..j].ends_with(c)
                && !text[..j].ends_with(char::is_whitespace)
                && !text[..j].ends_with('\\')
                && !(c == '_' && in_word(text[j + marker.len()..].chars().next()))
            {
                return Some((open, j, style));
            }
            j += 1;
        }
    }
    None
}
