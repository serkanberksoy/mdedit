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

/// Parses inline markup in `text`, returning styled spans with the syntax
/// hidden. `base` is patched with each token's style; markup nests
/// (`~~a **b** c~~`).
pub fn inline_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    parse_inline(text, base, &mut out);
    out
}

fn parse_inline(text: &str, base: Style, out: &mut Vec<Span<'static>>) {
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
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let prev = text[..i].chars().next_back();

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
                parse_inline(label, style, out);
            }
            i += len;
            continue;
        }

        // Reference link `[text][ref]` / `[text][]` (K-07): the text.
        if !rest.starts_with("[[")
            && let Some((len, label)) = reference_link(rest)
        {
            flush(&mut plain, out);
            parse_inline(label, base.patch(link), out);
            i += len;
            continue;
        }

        // Atomic tokens: shown with their own style, content not re-parsed.
        let token: Option<(usize, String, Style)> = if rest.starts_with("![[") {
            // An embed shown inline (not expanded): a marker and the name.
            rest.find("]]").map(|end| {
                let shown = link_text(&rest[3..end]);
                (end + 2, format!("⧉ {shown}"), link)
            })
        } else if rest.starts_with("[[") {
            rest.find("]]")
                .map(|end| (end + 2, link_text(&rest[2..end]), link))
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
            i += len;
            continue;
        }

        // Paired markers: the content is parsed again with the added style.
        if let Some((open, close, style)) = delimited(text, i) {
            flush(&mut plain, out);
            parse_inline(&text[open..close], base.patch(style), out);
            i = close + (open - i);
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
fn link_text(inner: &str) -> String {
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
