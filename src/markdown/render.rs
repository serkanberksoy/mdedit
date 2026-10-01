//! Rendering one line for display: heading, list, task, quote and callout
//! looks, code and frontmatter lines, and the raw (cursor-line) view.

use super::*;

/// Terminals can't change font size, so heading "size" is conveyed with
/// weight, color and decoration that get quieter as the level increases.
pub fn heading_style(level: u8) -> Style {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    match level {
        1 => bold.fg(Color::Magenta).add_modifier(Modifier::UNDERLINED),
        2 => bold.fg(Color::Cyan),
        3 => bold.fg(Color::Yellow),
        4 => bold.fg(Color::Green),
        5 => bold.fg(Color::Blue),
        _ => Style::default()
            .add_modifier(Modifier::ITALIC)
            .fg(Color::Gray),
    }
}

/// [`heading_style`] with the user's color for that level, if set.
pub fn heading_style_with(level: u8, options: &Options) -> Style {
    let style = heading_style(level);
    match options
        .heading_colors
        .get(usize::from(level) - 1)
        .copied()
        .flatten()
    {
        Some(color) => style.fg(color),
        None => style,
    }
}

fn heading_prefix(level: u8) -> &'static str {
    match level {
        1 => "█ ",
        2 => "▌ ",
        3 => "▎ ",
        _ => "",
    }
}

pub fn bullet_glyph(level: usize) -> char {
    match level % 3 {
        0 => '•',
        1 => '◦',
        _ => '▪',
    }
}

/// Glyph, glyph style and text style for a task state. Covers the common
/// Obsidian "alternate checkbox" states; unknown states show as `[c]`.
fn task_look(state: char, options: &Options) -> (String, Style, Style) {
    let plain = Style::default();
    let fg = |c| Style::default().fg(c);
    let struck = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::CROSSED_OUT);
    let (glyph, glyph_style, text_style) = match state {
        ' ' => ("☐", fg(Color::Cyan), plain),
        'x' | 'X' => {
            let text = match options.done_style {
                DoneStyle::Strike => struck,
                DoneStyle::Grey => fg(Color::DarkGray),
            };
            ("☑", fg(Color::Green), text)
        }
        '-' => ("☒", fg(Color::DarkGray), struck),
        '.' => ("⦿", fg(Color::Gray), plain),
        '/' => ("◐", fg(Color::Yellow), plain),
        '>' => ("➜", fg(Color::Magenta), plain),
        '<' => ("⏲", fg(Color::Magenta), plain),
        '!' => ("⚑", fg(Color::Red), plain.add_modifier(Modifier::BOLD)),
        '?' => ("?", fg(Color::Yellow), plain),
        '*' => ("★", fg(Color::Yellow), plain),
        _ => return (format!("[{state}] "), fg(Color::Cyan), plain),
    };
    (format!("{glyph} "), glyph_style, text_style)
}

fn callout_color(kind: &str) -> Color {
    match kind.to_ascii_lowercase().as_str() {
        "todo" | "info" | "note" => Color::Blue,
        "tip" | "hint" | "important" | "success" | "check" | "done" => Color::Green,
        "danger" | "error" | "bug" | "failure" | "fail" | "missing" => Color::Red,
        "warning" | "caution" | "attention" | "question" | "help" | "faq" => Color::Yellow,
        "example" => Color::Magenta,
        "quote" | "cite" => Color::Gray,
        _ => Color::Cyan,
    }
}

/// Icon shown before a callout's title (B-04), like Obsidian's per-type
/// icons. Single-width symbols, not emoji, so every terminal draws them in
/// one cell. Unknown (custom, B-07) types get Obsidian's default pencil.
fn callout_icon(kind: &str) -> &'static str {
    match kind.to_ascii_lowercase().as_str() {
        "abstract" | "summary" | "tldr" => "≡",
        "info" => "ℹ",
        "todo" => "☑",
        "tip" | "hint" | "important" => "✦",
        "success" | "check" | "done" => "✔",
        "question" | "help" | "faq" => "?",
        "warning" | "caution" | "attention" => "⚠",
        "failure" | "fail" | "missing" => "✘",
        "danger" | "error" => "ϟ",
        "bug" => "※",
        "example" => "☰",
        "quote" | "cite" => "❝",
        _ => "✎", // note, and custom types
    }
}

fn guides(level: usize) -> Span<'static> {
    Span::styled("│ ".repeat(level), Style::default().fg(Color::DarkGray))
}

/// A line ready for display, plus the hanging indent for its soft-wrapped
/// continuation rows (the width of its marker prefix, e.g. `│ • `).
#[derive(Debug)]
pub struct Rendered {
    pub line: Line<'static>,
    pub indent: usize,
}

impl From<Line<'static>> for Rendered {
    fn from(line: Line<'static>) -> Self {
        Rendered { line, indent: 0 }
    }
}

/// Builds a rendered line from marker spans (`head`, which set the hanging
/// indent) followed by `text` with inline markup.
fn line_of(mut head: Vec<Span<'static>>, text: &str, base: Style) -> Rendered {
    let indent = head.iter().map(|s| s.content.width()).sum();
    head.extend(inline_spans(text, base));
    Rendered {
        line: Line::from(head),
        indent,
    }
}

/// Fully rendered view of a line: syntax markers hidden, styling applied.
pub fn render_line(line: &str) -> Line<'static> {
    render(line).line
}

/// [`render_line`] plus the hanging indent for soft wrap.
pub fn render(line: &str) -> Rendered {
    render_with(line, &Options::default())
}

/// [`render`] with the user's rendering options.
pub fn render_with(line: &str, options: &Options) -> Rendered {
    match parse_line(line) {
        Block::Heading { level, text } => {
            let style = heading_style_with(level, options);
            let mut l = line_of(
                vec![Span::styled(heading_prefix(level), style)],
                text,
                style,
            );
            if level == 1 {
                for s in &mut l.line.spans {
                    s.content = s.content.to_uppercase().into();
                }
            }
            l
        }
        Block::Bullet {
            indent,
            marker,
            text,
        } => {
            let level = nesting(&line[..indent], options.indent_width);
            let shown = if is_ordered(marker) {
                format!("{marker} ")
            } else {
                format!("{} ", bullet_glyph(level))
            };
            let glyph = Span::styled(shown, Style::default().fg(Color::Cyan));
            line_of(vec![guides(level), glyph], text, Style::default())
        }
        Block::Task {
            indent,
            marker,
            state,
            text,
        } => {
            let (glyph, glyph_style, text_style) = task_look(state, options);
            let level = nesting(&line[..indent], options.indent_width);
            let mut head = vec![guides(level)];
            if is_ordered(marker) {
                head.push(Span::styled(
                    format!("{marker} "),
                    Style::default().fg(Color::Cyan),
                ));
            }
            head.push(Span::styled(glyph, glyph_style));
            line_of(head, text, text_style)
        }
        Block::Quote { depth, text } => {
            // Standalone: only the line's own header (if any) is known.
            let mut callouts = vec![None; depth];
            if let Some(body) = text.strip_prefix("[!")
                && let Some((kind, _)) = body.split_once(']')
            {
                callouts[depth - 1] = Some(kind.to_string());
            }
            render_quote(line, &callouts, None)
        }
        Block::Rule => Line::from(Span::styled(
            "─".repeat(40),
            Style::default().fg(Color::DarkGray),
        ))
        .into(),
        Block::Paragraph(text) => {
            let ws = text.chars().take_while(|&c| c == ' ' || c == '\t').count();
            let lead = Span::raw(text[..ws].replace('\t', TAB));
            line_of(vec![lead], &text[ws..], Style::default())
        }
    }
}

/// Rendered continuation line of a list item (L-07): the item's indent
/// guides, then the text lined up with the item's text (which is also the
/// hanging indent when it wraps).
pub fn render_continuation(line: &str, item: &str, options: &Options) -> Rendered {
    let level = match parse_line(item) {
        Block::Bullet { indent, .. } | Block::Task { indent, .. } => {
            nesting(&item[..indent], options.indent_width)
        }
        _ => 0,
    };
    let text_column = render(item).indent;
    let pad = " ".repeat(text_column.saturating_sub(2 * level));
    line_of(
        vec![guides(level), Span::raw(pad)],
        line.trim_start(),
        Style::default(),
    )
}

/// Rendered opening code fence: a frame corner plus the language, if any.
pub fn render_fence_open(lang: &str) -> Line<'static> {
    let label = if lang.is_empty() {
        "╭─".to_string()
    } else {
        format!("╭─ {lang}")
    };
    Line::from(Span::styled(label, Style::default().fg(Color::DarkGray)))
}

/// Rendered code line: literal text (no Markdown parsing) behind a frame bar.
pub fn render_code_line(line: &str) -> Rendered {
    let mut spans = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
    if !line.is_empty() {
        spans.push(Span::styled(
            line.replace('\t', TAB),
            Style::default().fg(CODE),
        ));
    }
    Rendered {
        line: Line::from(spans),
        indent: 2,
    }
}

/// Rendered code line from syntax-highlighted pieces (B-09).
pub fn render_code_pieces(pieces: &[(Style, String)]) -> Rendered {
    let mut spans = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
    spans.extend(
        pieces
            .iter()
            .map(|(style, text)| Span::styled(text.replace('\t', TAB), *style)),
    );
    Rendered {
        line: Line::from(spans),
        indent: 2,
    }
}

/// Rendered closing code fence.
pub fn render_fence_close() -> Line<'static> {
    Line::from(Span::styled("╰─", Style::default().fg(Color::DarkGray)))
}

/// Rendered quote line: one bar per depth, colored by the callout that
/// depth belongs to (`callouts[d]`, grey for a plain quote), then either a
/// callout header's title (in the callout's color) or the quoted text.
pub fn render_quote(line: &str, callouts: &[Option<String>], code: Option<&QuoteCode>) -> Rendered {
    let Block::Quote { depth, text } = parse_line(line) else {
        return render(line);
    };
    let bar_color = |d: usize| {
        callouts
            .get(d)
            .and_then(|k| k.as_deref())
            .map_or(Color::DarkGray, callout_color)
    };
    let mut bars: Vec<Span<'static>> = Vec::new();
    for d in 0..depth {
        let style = Style::default().fg(bar_color(d));
        match bars.last_mut() {
            Some(last) if last.style == style => last.content.to_mut().push_str("┃ "),
            _ => bars.push(Span::styled("┃ ", style)),
        }
    }
    // Fenced code inside the quote: the code frame after the bars.
    let dim = Style::default().fg(Color::DarkGray);
    match code {
        Some(QuoteCode::Open(lang)) => {
            let label = if lang.is_empty() {
                "╭─".to_string()
            } else {
                format!("╭─ {lang}")
            };
            bars.push(Span::styled(label, dim));
            return Line::from(bars).into();
        }
        Some(QuoteCode::Close) => {
            bars.push(Span::styled("╰─", dim));
            return Line::from(bars).into();
        }
        Some(QuoteCode::Body) => {
            bars.push(Span::styled("│ ", dim));
            let indent = bars.iter().map(|s| s.content.width()).sum();
            if !text.is_empty() {
                bars.push(Span::styled(
                    text.replace('\t', TAB),
                    Style::default().fg(CODE),
                ));
            }
            return Rendered {
                line: Line::from(bars),
                indent,
            };
        }
        None => {}
    }
    // Callout header: `[!kind]` optionally followed by `+`/`-` and a title.
    if let Some(body) = text.strip_prefix("[!")
        && let Some((kind, title)) = body.split_once(']')
    {
        let title = title.trim_start_matches(['+', '-']).trim();
        let style = Style::default()
            .fg(callout_color(kind))
            .add_modifier(Modifier::BOLD);
        let label = if title.is_empty() {
            kind.to_uppercase()
        } else {
            title.to_string()
        };
        let label = format!("{} {label}", callout_icon(kind));
        return line_of(bars, &label, style);
    }
    line_of(bars, text, Style::default().fg(Color::Gray))
}

/// Rendered view of a line inside the frontmatter block (shown dimmed, like
/// a collapsed properties panel).
/// A link reference definition (K-07), dimmed except for its target:
/// `[docs]: https://example.com "Title"`.
pub fn render_link_definition(line: &str) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    let Some((_, target, title)) = crate::links::link_definition(line) else {
        return Line::from(Span::styled(line.to_string(), dim));
    };
    // `target` and `title` are slices of `line`: split it around them.
    let start = target.as_ptr() as usize - line.as_ptr() as usize;
    let web = matches!(
        crate::links::target_link(target),
        crate::links::Link::Web(_)
    );
    let style = if web {
        Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::LightBlue)
    };
    let mut spans = vec![
        Span::styled(line[..start].to_string(), dim),
        Span::styled(target.to_string(), style),
    ];
    if !title.is_empty() {
        spans.push(Span::styled(title.to_string(), dim));
    }
    Line::from(spans)
}

/// A line of a comment block (X-08): dimmed, its text in italics.
pub fn render_comment_line(line: &str) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    if line.trim() == "%%" {
        return Line::from(Span::styled(line.to_string(), dim));
    }
    Line::from(Span::styled(
        line.to_string(),
        dim.add_modifier(Modifier::ITALIC),
    ))
}

/// A line of the frontmatter as a property (P-01, P-02, P-03, P-06): the
/// key, then its value by type: a checkbox `☑` / `☐`, a number, a date,
/// a list as chips, `tags` as tags, text with its links and tags styled.
/// A list item (`  - x`) under key `owner` is a chip (a tag under `tags`).
pub fn render_frontmatter_line(line: &str, owner: Option<&str>) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    if line.trim_end() == "---" {
        return Line::from(Span::styled("┄".repeat(40), dim));
    }
    let indented = line.starts_with([' ', '\t']);
    if let Some(item) = line.trim_start().strip_prefix("- ") {
        let indent = line.len() - line.trim_start().len();
        let mut spans = vec![Span::raw(" ".repeat(indent.max(2)))];
        spans.push(property_chip(
            owner.unwrap_or_default(),
            unquote(item.trim()),
        ));
        return Line::from(spans);
    }
    match line.split_once(':') {
        Some((key, value)) if !indented => {
            let mut spans = vec![Span::styled(
                key.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
            )];
            let value = property_value(key.trim(), value.trim());
            if !value.is_empty() {
                spans.push(Span::raw(" "));
                spans.extend(value);
            }
            Line::from(spans)
        }
        _ => Line::from(Span::styled(line.replace('\t', TAB), dim)),
    }
}

/// A value without its quotes.
fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
}

/// One item of a list property: a tag under `tags`, else a chip.
fn property_chip(key: &str, item: &str) -> Span<'static> {
    if matches!(key, "tags" | "tag") {
        let tag = item.trim_start_matches('#');
        return Span::styled(
            format!("#{tag}"),
            Style::default()
                .fg(Color::LightCyan)
                .bg(Color::Rgb(25, 55, 100)),
        );
    }
    Span::styled(
        item.to_string(),
        Style::default().fg(Color::White).bg(Color::Rgb(60, 60, 60)),
    )
}

/// Whether `value` looks like a date (`2026-06-29`, with a time too).
fn is_date(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..10]
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// A property's value as styled spans, by its type.
fn property_value(key: &str, value: &str) -> Vec<Span<'static>> {
    if value.is_empty() {
        return Vec::new();
    }
    let list = value.strip_prefix('[').and_then(|v| v.strip_suffix(']'));
    let tags = matches!(key, "tags" | "tag");
    if list.is_some() || tags {
        let items: Vec<&str> = list
            .unwrap_or(value)
            .split(if list.is_some() { ',' } else { ' ' })
            .map(|i| unquote(i.trim()))
            .filter(|i| !i.is_empty())
            .collect();
        let mut spans = Vec::new();
        for (n, item) in items.iter().enumerate() {
            if n > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(property_chip(key, item));
        }
        return spans;
    }
    match value {
        "true" => return vec![Span::styled("☑", Style::default().fg(Color::Green))],
        "false" => return vec![Span::styled("☐", Style::default().fg(Color::DarkGray))],
        _ => {}
    }
    if value.parse::<f64>().is_ok() {
        return vec![Span::styled(
            value.to_string(),
            Style::default().fg(Color::Yellow),
        )];
    }
    if is_date(value) {
        return vec![Span::styled(
            value.to_string(),
            Style::default().fg(Color::LightMagenta),
        )];
    }
    inline::inline_spans(unquote(value), Style::default())
}

/// Byte length of a task's syntax prefix (`  - [ ] `), capped at the line length.
pub fn task_prefix_len(line: &str, indent: usize, marker: &str, state: char) -> usize {
    (indent + marker.len() + 4 + state.len_utf8()).min(line.len())
}

/// Raw source view of a line (used for the line under the cursor): the text is
/// shown verbatim (tabs expanded to [`TAB`]) so columns match the buffer, with
/// markers dimmed.
pub fn render_source_line(line: &str) -> Line<'static> {
    render_source(line).line
}

/// [`render_source_line`] plus the hanging indent: continuation rows line up
/// with the text after the marker (`- [ ] `, `## `, `> `).
pub fn render_source(line: &str) -> Rendered {
    render_source_with(line, &Options::default())
}

/// [`render_source`] with the user's rendering options.
pub fn render_source_with(line: &str, options: &Options) -> Rendered {
    let marker = Style::default().fg(Color::DarkGray);
    let span = |s: &str, style| Span::styled(s.replace('\t', TAB), style);
    let split = |at: usize, body: Style| {
        let (head, rest) = line.split_at(at);
        Rendered {
            line: Line::from(vec![span(head, marker), span(rest, body)]),
            indent: head.replace('\t', TAB).width() + usize::from(rest.starts_with(' ')),
        }
    };
    match parse_line(line) {
        Block::Heading { level, .. } => split(level as usize, heading_style_with(level, options)),
        Block::Bullet { indent, marker, .. } => split(
            (indent + marker.len() + 1).min(line.len()),
            Style::default(),
        ),
        Block::Task {
            indent,
            marker,
            state,
            ..
        } => {
            let body = match (is_done(state), options.done_style) {
                (true, DoneStyle::Strike) => Style::default().add_modifier(Modifier::CROSSED_OUT),
                (true, DoneStyle::Grey) => Style::default().fg(Color::DarkGray),
                (false, _) => Style::default(),
            };
            split(task_prefix_len(line, indent, marker, state), body)
        }
        Block::Quote { .. } => {
            let (_, prefix) = quote_prefix(line).expect("a quote has a prefix");
            split(prefix, Style::default())
        }
        Block::Rule => Line::from(span(line, marker)).into(),
        Block::Paragraph(_) => {
            let ws = line.len() - line.trim_start_matches([' ', '\t']).len();
            Rendered {
                line: Line::from(span(line, Style::default())),
                indent: line[..ws].replace('\t', TAB).width(),
            }
        }
    }
}
