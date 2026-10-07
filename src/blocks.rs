//! Document-level block structure: which lines belong to multi-line blocks
//! (frontmatter, fenced code, callouts, setext headings). Computed in
//! O(lines) and cached by the text ([`analyze_cached`]), because a key press
//! needs it several times (movement, folds, drawing).

use crate::markdown::{
    Align, Block, cell_width, parse_line, render, table_aligns, table_cells, table_separator,
};

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// What a line is, in the context of the whole document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineContext {
    /// Not part of a multi-line block; rendered by the line parser.
    Normal,
    /// Inside the leading `---` … `---` YAML block (fence lines included).
    Frontmatter,
    /// Opening code fence; `lang` is the info string (may be empty).
    FenceOpen {
        lang: String,
    },
    FenceBody,
    FenceClose,
    /// A `>` line. `callouts[d]` is the callout (kind) that depth `d + 1`
    /// belongs to, or `None` for a plain quote at that depth.
    Quote {
        callouts: Vec<Option<String>>,
        /// Part of a fenced code block inside the quote (B-08).
        code: Option<QuoteCode>,
    },
    /// Text of a setext heading (the line above a `===` / `---` underline).
    SetextHeading {
        level: u8,
    },
    /// The `===` (level 1) or `---` (level 2) underline; `width` is the
    /// rendered heading's display width, so the underline can match it.
    SetextUnderline {
        level: u8,
        width: usize,
    },
    /// Indented text belonging to the list item on line `owner` (L-07).
    ListContinuation {
        owner: usize,
    },
    /// A line of an indented code block (4+ columns, after a blank line,
    /// outside lists; B-10).
    IndentedCode,
    /// A link reference definition, `[label]: url` (K-07).
    LinkDefinition,
    /// A line of a `%%` … `%%` comment block (X-08), its `%%` lines too.
    Comment,
    /// A row of table `table` (index into `Structure::tables`; B-11).
    TableRow {
        table: usize,
        kind: TableRowKind,
    },
}

/// The role of a quote line in a fenced code block inside the quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoteCode {
    /// Opening fence; the info string's first word.
    Open(String),
    Body,
    Close,
}

/// Which row of a table a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRowKind {
    Header,
    /// The `|---|---|` line.
    Separator,
    Body,
}

/// The column widths of a table's lines (its second line the separator):
/// each column's widest cell.
pub(crate) fn table_widths(lines: &[String]) -> Vec<usize> {
    let mut widths = vec![1; lines.first().map_or(0, |h| table_cells(h).len())];
    for (_, line) in lines.iter().enumerate().filter(|&(r, _)| r != 1) {
        for (w, cell) in widths.iter_mut().zip(table_cells(line)) {
            *w = (*w).max(cell_width(cell));
        }
    }
    widths
}

/// A table's lines and column widths (display width of the widest
/// rendered cell in each column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub start: usize,
    pub end: usize,
    pub widths: Vec<usize>,
    /// Per column, from the separator row (B-12).
    pub aligns: Vec<Align>,
}

/// Per-line context plus the line ranges that are revealed as a whole.
#[derive(Debug, Default)]
pub struct Structure {
    pub context: Vec<LineContext>,
    /// Inclusive line ranges shown raw together when the cursor is inside
    /// any of their lines (frontmatter, fenced code blocks).
    pub reveal_groups: Vec<(usize, usize)>,
    /// Foldable callouts (`> [!kind]-` / `+`), in document order.
    pub folds: Vec<Fold>,
    /// Tables (B-11), in document order.
    pub tables: Vec<Table>,
}

/// What a fold belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    /// A callout with `+` / `-` (B-05).
    Callout,
    /// A heading's section (V-06).
    Section,
    /// A list item's sub-items and continuation lines (V-07).
    List,
}

/// Something Ctrl+K can fold: its header line (which stays visible), the
/// hidden lines `start..=end`, and whether it starts collapsed (only
/// callouts marked `-` do).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fold {
    pub header: usize,
    pub start: usize,
    pub end: usize,
    pub collapsed: bool,
    pub kind: FoldKind,
}

impl Structure {
    /// Whether `fold` is collapsed, given the user's Ctrl+K choices
    /// (`overrides`, by header line) over the `+`/`-` default.
    pub fn is_collapsed(&self, fold: &Fold, overrides: &HashMap<usize, bool>) -> bool {
        overrides
            .get(&fold.header)
            .copied()
            .unwrap_or(fold.collapsed)
    }

    /// The collapsed folds whose body contains `row` (so `row` is hidden).
    pub fn hiding<'a>(
        &'a self,
        row: usize,
        overrides: &'a HashMap<usize, bool>,
    ) -> impl Iterator<Item = &'a Fold> {
        self.folds
            .iter()
            .filter(move |f| f.start <= row && row <= f.end && self.is_collapsed(f, overrides))
    }

    /// The innermost fold containing `row` (header or body).
    pub fn fold_around(&self, row: usize) -> Option<&Fold> {
        self.folds
            .iter()
            .rev()
            .find(|f| f.header <= row && row <= f.end)
    }

    /// The level and text of line `i` if it's a heading: an ATX heading
    /// outside any block (not a `#` comment in code), or a setext heading's
    /// text line.
    pub fn heading<'a>(&self, lines: &'a [String], i: usize) -> Option<(u8, &'a str)> {
        match (&self.context[i], parse_line(&lines[i])) {
            (LineContext::SetextHeading { level }, _) => Some((*level, lines[i].trim())),
            (LineContext::Normal, Block::Heading { level, text }) => Some((level, text)),
            _ => None,
        }
    }

    /// The reveal group containing `row`, if any.
    pub fn reveal_group(&self, row: usize) -> Option<(usize, usize)> {
        self.reveal_groups
            .iter()
            .copied()
            .find(|&(start, end)| (start..=end).contains(&row))
    }
}

/// An opening code fence: its char (`` ` `` or `~`), length and info string.
fn fence_open(line: &str) -> Option<(char, usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = rest.chars().take_while(|&c| c == ch).count();
    let info = rest[len..].trim();
    // Backtick fences can't have backticks in the info string (CommonMark).
    (len >= 3 && !(ch == '`' && info.contains('`'))).then_some((ch, len, info))
}

/// Whether `line` closes a fence opened with `ch` repeated `len` times.
fn fence_closes(line: &str, ch: char, len: usize) -> bool {
    fence_open(line).is_some_and(|(c, n, info)| c == ch && n >= len && info.is_empty())
}

/// Leading indentation of `line` in columns (a tab counts as 4).
pub fn indent_columns(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// The `kind` of a callout header's text (`[!kind] Title` → `kind`).
pub fn callout_header(text: &str) -> Option<&str> {
    let body = text.strip_prefix("[!")?;
    body.split_once(']').map(|(kind, _)| kind)
}

/// [`analyze`], memoized by the text. Hashing the lines is much cheaper
/// than analyzing them; the cache holds the main note and its embeds.
pub fn analyze_cached(lines: &[String]) -> Arc<Structure> {
    const LIMIT: usize = 16;
    static CACHE: OnceLock<Mutex<HashMap<u64, Arc<Structure>>>> = OnceLock::new();
    let mut hasher = DefaultHasher::new();
    lines.hash(&mut hasher);
    let key = hasher.finish();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let structure = Arc::new(analyze(lines));
    if let Ok(mut c) = cache.lock() {
        if c.len() >= LIMIT {
            c.clear();
        }
        c.insert(key, Arc::clone(&structure));
    }
    structure
}

pub fn analyze(lines: &[String]) -> Structure {
    let mut s = Structure {
        context: vec![LineContext::Normal; lines.len()],
        reveal_groups: Vec::new(),
        folds: Vec::new(),
        tables: Vec::new(),
    };
    let mut i = 0;

    if lines.first().map(|l| l.trim_end()) == Some("---")
        && let Some(end) = lines.iter().skip(1).position(|l| l.trim_end() == "---")
    {
        let end = end + 1;
        s.context[..=end].fill(LineContext::Frontmatter);
        s.reveal_groups.push((0, end));
        i = end + 1;
    }

    // Callout kind per quote depth in the current quote run (B-06).
    let mut callouts: Vec<Option<String>> = Vec::new();
    // A fenced code block inside the quote: (depth, char, length, start).
    let mut quote_fence: Option<(usize, char, usize, usize)> = None;
    // Open foldable callouts: (depth, index into `s.folds`).
    let mut open_folds: Vec<(usize, usize)> = Vec::new();
    let close = |folds: &mut Vec<Fold>, open: &mut Vec<(usize, usize)>, keep: usize, end: usize| {
        while open.last().is_some_and(|&(d, _)| d > keep) {
            let (_, k) = open.pop().expect("checked above");
            folds[k].end = end;
        }
    };
    // The list item that indented lines below belong to (L-07).
    let mut item: Option<usize> = None;
    // The indented code block being read: (first line, last code line).
    let mut code_block: Option<(usize, usize)> = None;
    let end_code = |s: &mut Structure, block: &mut Option<(usize, usize)>| {
        if let Some((start, end)) = block.take() {
            s.reveal_groups.push((start, end));
        }
    };
    while i < lines.len() {
        let line = &lines[i];
        // A comment block (X-08): from a `%%` line to the next.
        if line.trim() == "%%" {
            let start = i;
            s.context[i] = LineContext::Comment;
            i += 1;
            while i < lines.len() && lines[i].trim() != "%%" {
                s.context[i] = LineContext::Comment;
                i += 1;
            }
            let end = i.min(lines.len() - 1);
            s.context[end] = LineContext::Comment;
            s.reveal_groups.push((start, end));
            end_code(&mut s, &mut code_block);
            item = None;
            i += 1;
            continue;
        }
        if let Some((ch, len, info)) = fence_open(line) {
            let lang = info
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            s.context[i] = LineContext::FenceOpen { lang };
            let start = i;
            i += 1;
            while i < lines.len() && !fence_closes(&lines[i], ch, len) {
                s.context[i] = LineContext::FenceBody;
                i += 1;
            }
            let end = if i < lines.len() {
                s.context[i] = LineContext::FenceClose;
                i
            } else {
                lines.len() - 1
            };
            s.reveal_groups.push((start, end));
            end_code(&mut s, &mut code_block);
            callouts.clear();
            close(&mut s.folds, &mut open_folds, 0, start.saturating_sub(1));
            item = None;
            i += 1;
            continue;
        }

        let after_break = i == 0
            || lines[i - 1].trim().is_empty()
            || s.context[i - 1] == LineContext::IndentedCode;
        // Parsed once: lists and quotes below both need it.
        let parsed = parse_line(line);
        if line.starts_with('>') {
            item = None;
            end_code(&mut s, &mut code_block);
        } else if line.trim().is_empty() {
            // Blank lines don't end lists or code blocks.
        } else if item.is_none() && indent_columns(line) >= 4 && after_break {
            // Indented code (B-10): outside lists, after a blank line.
            s.context[i] = LineContext::IndentedCode;
            code_block = Some((code_block.map_or(i, |(start, _)| start), i));
        } else {
            end_code(&mut s, &mut code_block);
            if matches!(parsed, Block::Bullet { .. } | Block::Task { .. }) {
                item = Some(i);
            } else if let Some(owner) = item.filter(|_| line.starts_with([' ', '\t'])) {
                s.context[i] = LineContext::ListContinuation { owner };
            } else {
                item = None;
            }
        }

        match parsed {
            Block::Quote { depth, text } => {
                callouts.truncate(depth);
                callouts.resize(depth, None);
                // Fenced code inside the quote (B-08).
                let mut code = None;
                if let Some((fence_depth, ch, len, start)) = quote_fence {
                    if depth < fence_depth {
                        s.reveal_groups.push((start, i - 1));
                        quote_fence = None;
                    } else if fence_closes(text, ch, len) {
                        code = Some(QuoteCode::Close);
                        s.reveal_groups.push((start, i));
                        quote_fence = None;
                    } else {
                        code = Some(QuoteCode::Body);
                    }
                }
                if code.is_none()
                    && let Some((ch, len, info)) = fence_open(text)
                {
                    let lang = info.split_whitespace().next().unwrap_or_default();
                    code = Some(QuoteCode::Open(lang.to_string()));
                    quote_fence = Some((depth, ch, len, i));
                }
                let header = callout_header(text).filter(|_| code.is_none());
                // A shallower line, or a new header at this depth, ends folds.
                let keep = if header.is_some() { depth - 1 } else { depth };
                close(&mut s.folds, &mut open_folds, keep, i.saturating_sub(1));
                if let Some(kind) = header {
                    callouts[depth - 1] = Some(kind.to_string());
                    let sign = text[2 + kind.len() + 1..].chars().next();
                    if let Some(sign @ ('+' | '-')) = sign {
                        open_folds.push((depth, s.folds.len()));
                        s.folds.push(Fold {
                            header: i,
                            start: i + 1,
                            end: i,
                            collapsed: sign == '-',
                            kind: FoldKind::Callout,
                        });
                    }
                }
                s.context[i] = LineContext::Quote {
                    callouts: callouts.clone(),
                    code,
                };
            }
            _ => {
                callouts.clear();
                if let Some((_, _, _, start)) = quote_fence.take() {
                    s.reveal_groups.push((start, i - 1));
                }
                close(&mut s.folds, &mut open_folds, 0, i.saturating_sub(1));
            }
        }
        i += 1;
    }
    close(
        &mut s.folds,
        &mut open_folds,
        0,
        lines.len().saturating_sub(1),
    );
    if let Some((_, _, _, start)) = quote_fence {
        s.reveal_groups.push((start, lines.len() - 1));
    }
    end_code(&mut s, &mut code_block);

    // Tables (B-11): a row with pipes above a matching separator row, then
    // rows with pipes until a blank line or a line without one.
    let mut i = 0;
    while i + 1 < lines.len() {
        let normal = |s: &Structure, k: usize| s.context[k] == LineContext::Normal;
        let header = &lines[i];
        // The cheap checks first: most lines have no pipe.
        if !(header.contains('|') && normal(&s, i) && normal(&s, i + 1))
            || table_separator(&lines[i + 1]) != Some(table_cells(header).len())
        {
            i += 1;
            continue;
        }
        let table = s.tables.len();
        let row = |kind| LineContext::TableRow { table, kind };
        s.context[i] = row(TableRowKind::Header);
        s.context[i + 1] = row(TableRowKind::Separator);
        let mut end = i + 1;
        while end + 1 < lines.len() && normal(&s, end + 1) && lines[end + 1].contains('|') {
            end += 1;
            s.context[end] = row(TableRowKind::Body);
        }
        let widths = table_widths(&lines[i..=end]);
        s.tables.push(Table {
            start: i,
            end,
            widths,
            aligns: table_aligns(&lines[i + 1]),
        });
        s.reveal_groups.push((i, end));
        i = end + 1;
    }

    // Link reference definitions (K-07).
    for (i, line) in lines.iter().enumerate() {
        if s.context[i] == LineContext::Normal && crate::links::link_definition(line).is_some() {
            s.context[i] = LineContext::LinkDefinition;
        }
    }

    // Setext headings: a paragraph line directly above a `===` / `---` line.
    for i in 0..lines.len().saturating_sub(1) {
        let normal = |k: usize| s.context[k] == LineContext::Normal;
        if normal(i)
            && normal(i + 1)
            && let Some(level) = setext_underline(&lines[i + 1])
            && is_paragraph_text(&lines[i])
        {
            let heading = format!("{} {}", "#".repeat(level as usize), lines[i].trim());
            let width = render(&heading).line.width();
            s.context[i] = LineContext::SetextHeading { level };
            s.context[i + 1] = LineContext::SetextUnderline { level, width };
            s.reveal_groups.push((i, i + 1));
        }
    }
    s.reveal_groups.sort_unstable();
    add_section_and_list_folds(lines, &mut s);
    s
}

/// Folds for heading sections (V-06) and list items with children (V-07),
/// all open by default. One pass with a stack of open sections and one of
/// open list items, so deep nesting stays linear. Folds end up sorted by
/// header line, so the last one containing a line is the innermost.
fn add_section_and_list_folds(lines: &[String], s: &mut Structure) {
    let mut folds = Vec::new();
    // Open sections: (heading line, level, first body line).
    let mut sections: Vec<(usize, u8, usize)> = Vec::new();
    // Open list items: (item line, its indent).
    let mut items: Vec<(usize, usize)> = Vec::new();
    // The last non-blank line so far: where a closed list item ends.
    let mut last_text: Option<usize> = None;
    let close_item = |folds: &mut Vec<Fold>, header: usize, last: Option<usize>| {
        // A list item's children are the following lines indented deeper
        // (blank lines between them included).
        if let Some(end) = last.filter(|&end| end > header) {
            folds.push(Fold {
                header,
                start: header + 1,
                end,
                collapsed: false,
                kind: FoldKind::List,
            });
        }
    };
    // A section runs to the next heading of the same or a higher level.
    let close_section = |folds: &mut Vec<Fold>, (header, _, start): (usize, u8, usize), end| {
        if start < end {
            folds.push(Fold {
                header,
                start,
                end: end - 1,
                collapsed: false,
                kind: FoldKind::Section,
            });
        }
    };
    for (i, line) in lines.iter().enumerate() {
        if let Some((level, _)) = s.heading(lines, i) {
            while let Some(&open) = sections.last().filter(|&&(_, l, _)| l >= level) {
                sections.pop();
                close_section(&mut folds, open, i);
            }
            let setext = matches!(s.context[i], LineContext::SetextHeading { .. });
            sections.push((i, level, i + if setext { 2 } else { 1 }));
        }
        if line.trim().is_empty() {
            continue;
        }
        let indent = indent_columns(line);
        while let Some(&(header, _)) = items.last().filter(|&&(_, own)| own >= indent) {
            items.pop();
            close_item(&mut folds, header, last_text);
        }
        if s.context[i] == LineContext::Normal
            && matches!(parse_line(line), Block::Bullet { .. } | Block::Task { .. })
        {
            items.push((i, indent));
        }
        last_text = Some(i);
    }
    while let Some((header, _)) = items.pop() {
        close_item(&mut folds, header, last_text);
    }
    while let Some(open) = sections.pop() {
        close_section(&mut folds, open, lines.len());
    }
    s.folds.extend(folds);
    s.folds.sort_by_key(|f| f.header);
}

/// Whether `line` can be the text of a setext heading: a non-blank paragraph
/// line (not a list item, quote, heading or rule), indented less than 4.
fn is_paragraph_text(line: &str) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    indent < 4 && !line.trim().is_empty() && matches!(parse_line(line), Block::Paragraph(_))
}

/// The heading level of a setext underline: `=` → 1, `-` → 2 (any length,
/// up to 3 spaces of indent, trailing spaces allowed).
fn setext_underline(line: &str) -> Option<u8> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = line.trim();
    if indent > 3 || rest.is_empty() {
        return None;
    }
    if rest.chars().all(|c| c == '=') {
        Some(1)
    } else if rest.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LineContext::*;

    fn ctx(text: &str) -> Vec<LineContext> {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        analyze(&lines).context
    }

    fn open(lang: &str) -> LineContext {
        FenceOpen { lang: lang.into() }
    }

    #[test]
    fn plain_lines_are_normal() {
        assert_eq!(ctx("# a\n- b\ntext"), [Normal, Normal, Normal]);
    }

    #[test]
    fn frontmatter_only_at_top() {
        assert_eq!(
            ctx("---\nk: v\n---\n---\nx"),
            [Frontmatter, Frontmatter, Frontmatter, Normal, Normal]
        );
        // Not at the top: no frontmatter (these pairs are setext headings).
        assert!(!ctx("x\n---\nk: v\n---").contains(&Frontmatter));
        // Unclosed: not frontmatter.
        assert_eq!(ctx("---\nk: v"), [Normal, Normal]);
    }

    #[test]
    fn backtick_and_tilde_fences() {
        assert_eq!(
            ctx("```rust\nlet x;\n```\n~~~\n# not heading\n~~~\nafter"),
            [
                open("rust"),
                FenceBody,
                FenceClose,
                open(""),
                FenceBody,
                FenceClose,
                Normal
            ]
        );
    }

    #[test]
    fn closing_fence_must_match_char_and_length() {
        assert_eq!(
            ctx("````\n```\n~~~~\n`````\nx"),
            [open(""), FenceBody, FenceBody, FenceClose, Normal]
        );
    }

    #[test]
    fn closing_fence_has_no_info_string() {
        assert_eq!(ctx("```\n```js\n```"), [open(""), FenceBody, FenceClose]);
    }

    #[test]
    fn unclosed_fence_runs_to_end() {
        assert_eq!(ctx("```\na\nb"), [open(""), FenceBody, FenceBody]);
    }

    #[test]
    fn fence_indented_up_to_three_spaces() {
        assert_eq!(ctx("   ```\nx\n   ```"), [open(""), FenceBody, FenceClose]);
        // Indented 4 or more it isn't a fence (it's indented code, B-10).
        assert!(!matches!(ctx("    ```\nx")[0], FenceOpen { .. }));
    }

    #[test]
    fn fence_info_string_is_first_word() {
        assert_eq!(ctx("``` rust  extra\n```")[0], open("rust"));
    }

    #[test]
    fn frontmatter_like_lines_inside_fence_are_code() {
        assert_eq!(
            ctx("x\n```\n---\n```"),
            [Normal, open(""), FenceBody, FenceClose]
        );
    }

    /// A quote line whose depths belong to the given callouts (`None` =
    /// plain quote at that depth).
    fn q(bars: &[Option<&str>]) -> LineContext {
        qc(bars, None)
    }

    fn qc(bars: &[Option<&str>], code: Option<QuoteCode>) -> LineContext {
        Quote {
            callouts: bars.iter().map(|b| b.map(String::from)).collect(),
            code,
        }
    }

    #[test]
    fn fences_inside_quotes_and_callouts() {
        let todo = [Some("todo")];
        assert_eq!(
            ctx("> [!todo] T\n> ```tasks\n> not done\n> ```\nafter"),
            [
                q(&todo),
                qc(&todo, Some(QuoteCode::Open("tasks".into()))),
                qc(&todo, Some(QuoteCode::Body)),
                qc(&todo, Some(QuoteCode::Close)),
                Normal,
            ]
        );
        let lines: Vec<String> = ["> x", "> ```", "> a", "> ```"].map(String::from).to_vec();
        assert_eq!(analyze(&lines).reveal_group(2), Some((1, 3)));
    }

    #[test]
    fn quoted_fence_ends_with_the_quote() {
        assert_eq!(
            ctx("> ```\n> a\nout"),
            [
                qc(&[None], Some(QuoteCode::Open(String::new()))),
                qc(&[None], Some(QuoteCode::Body)),
                Normal,
            ]
        );
        let lines: Vec<String> = ["> ```", "> a", "out"].map(String::from).to_vec();
        assert_eq!(analyze(&lines).reveal_group(0), Some((0, 1)));
    }

    #[test]
    fn callout_body_follows_header_until_quote_ends() {
        let tip = || q(&[Some("tip")]);
        assert_eq!(
            ctx("> [!tip] T\n> a\n>\n> b\nafter\n> plain quote"),
            [tip(), tip(), tip(), tip(), Normal, q(&[None])]
        );
    }

    #[test]
    fn plain_quote_is_not_a_callout() {
        assert_eq!(ctx("> a\n> b"), [q(&[None]), q(&[None])]);
    }

    fn folds(text: &str) -> Vec<(usize, usize, bool)> {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        analyze(&lines)
            .folds
            .iter()
            .map(|f| (f.header, f.end, f.collapsed))
            .collect()
    }

    #[test]
    fn foldable_callouts_are_found_with_their_default() {
        assert_eq!(folds("> [!tip]- T\n> a\n> b\nafter"), [(0, 2, true)]);
        assert_eq!(folds("> [!tip]+ T\n> a"), [(0, 1, false)]);
        assert_eq!(folds("> [!tip] T\n> a"), [], "no sign: not foldable");
        assert_eq!(folds("> [!tip]- T"), [(0, 0, true)], "no body");
    }

    #[test]
    fn nested_foldable_callouts() {
        assert_eq!(
            folds("> [!note]- O\n> x\n> > [!tip]+ I\n> > y\n> z\nout"),
            [(0, 4, true), (2, 3, false)]
        );
    }

    fn fold_ranges(text: &str) -> Vec<(usize, usize, usize, FoldKind)> {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        analyze(&lines)
            .folds
            .iter()
            .map(|f| (f.header, f.start, f.end, f.kind))
            .collect()
    }

    #[test]
    fn sections_fold_up_to_the_next_heading_of_the_same_level() {
        use FoldKind::Section;
        assert_eq!(
            fold_ranges("# A\na\n## B\nb\n# C\nc"),
            [(0, 1, 3, Section), (2, 3, 3, Section), (4, 5, 5, Section)]
        );
        assert_eq!(fold_ranges("# Empty\n# Next"), [], "nothing to fold");
        assert!(
            folds("# A\na").iter().all(|&(_, _, collapsed)| !collapsed),
            "open by default"
        );
    }

    #[test]
    fn setext_sections_fold_below_their_underline() {
        assert_eq!(
            fold_ranges("Title\n===\nbody"),
            [(0, 2, 2, FoldKind::Section)]
        );
    }

    #[test]
    fn list_items_fold_their_children() {
        use FoldKind::List;
        assert_eq!(
            fold_ranges("- a\n  - a1\n    - a11\n  more a\n- b\n- c\n  - c1"),
            [(0, 1, 3, List), (1, 2, 2, List), (5, 6, 6, List)]
        );
    }

    #[test]
    fn callout_folds_start_after_the_header() {
        assert_eq!(
            fold_ranges("> [!tip]- T\n> a"),
            [(0, 1, 1, FoldKind::Callout)]
        );
    }

    #[test]
    fn nested_callouts_track_each_depth() {
        assert_eq!(
            ctx("> [!note] O\n> body\n> > [!tip] I\n> > inner\n> back out\n> > plain"),
            [
                q(&[Some("note")]),
                q(&[Some("note")]),
                q(&[Some("note"), Some("tip")]),
                q(&[Some("note"), Some("tip")]),
                q(&[Some("note")]),
                q(&[Some("note"), None]),
            ]
        );
    }

    #[test]
    fn list_continuations_belong_to_the_item_above() {
        let c = |owner| ListContinuation { owner };
        assert_eq!(
            ctx("- a\n  more\n\n  para\n- b\n  - n\n    deep\nafter"),
            [Normal, c(0), Normal, c(0), Normal, Normal, c(5), Normal]
        );
    }

    #[test]
    fn unindented_text_ends_the_list() {
        assert_eq!(ctx("- a\ntext\n  indented"), [Normal, Normal, Normal]);
        assert_eq!(ctx("  indented without a list"), [Normal]);
    }

    #[test]
    fn indented_code_after_a_blank_line() {
        assert_eq!(
            ctx("para\n\n    code\n\tmore\n\n    still\n\nafter"),
            [
                Normal,
                Normal,
                IndentedCode,
                IndentedCode,
                Normal,
                IndentedCode,
                Normal,
                Normal
            ]
        );
        let lines: Vec<String> = ["", "    a", "", "    b", "x"].map(String::from).to_vec();
        assert_eq!(analyze(&lines).reveal_group(1), Some((1, 3)));
        assert_eq!(ctx("    at the start"), [IndentedCode]);
    }

    #[test]
    fn indented_code_cannot_interrupt_a_paragraph_or_a_list() {
        assert_eq!(ctx("para\n    not code"), [Normal, Normal]);
        assert_eq!(
            ctx("- item\n\n    continuation"),
            [Normal, Normal, ListContinuation { owner: 0 }]
        );
        assert_eq!(ctx("- 10:00\n\t- [.] log"), [Normal, Normal]);
    }

    #[test]
    fn tables_need_a_matching_separator_row() {
        let t = |kind| TableRow { table: 0, kind };
        assert_eq!(
            ctx("| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n\nafter"),
            [
                t(TableRowKind::Header),
                t(TableRowKind::Separator),
                t(TableRowKind::Body),
                t(TableRowKind::Body),
                Normal,
                Normal
            ]
        );
        // No separator, or a different number of cells: not a table.
        assert_eq!(ctx("| a | b |\n| 1 | 2 |"), [Normal, Normal]);
        assert_eq!(ctx("| a | b |\n|---|"), [Normal, Normal]);
        // Outer pipes are optional.
        assert_eq!(ctx("a | b\n--- | ---")[1], t(TableRowKind::Separator));
    }

    #[test]
    fn a_table_ends_at_a_line_without_a_pipe() {
        let s = analyze(&["| a |", "|---|", "| 1 |", "text"].map(String::from));
        assert_eq!(s.context[3], Normal);
        assert_eq!(s.reveal_group(2), Some((0, 2)));
    }

    #[test]
    fn table_columns_are_as_wide_as_their_widest_rendered_cell() {
        let s =
            analyze(&["| Name | x |", "|---|---|", "| **mdedit** | [[bb]] |"].map(String::from));
        assert_eq!(s.tables[0].widths, [6, 2], "markup isn't counted");
    }

    #[test]
    fn table_alignment_comes_from_the_separator() {
        let s = analyze(&["a|b|c|d", ":--|:-:|--:|---"].map(String::from));
        assert_eq!(
            s.tables[0].aligns,
            [Align::Left, Align::Center, Align::Right, Align::Left]
        );
    }

    #[test]
    fn setext_headings() {
        let h = |level| SetextHeading { level };
        let u = |level, width| SetextUnderline { level, width };
        assert_eq!(ctx("Title\n==="), [h(1), u(1, 7)]);
        assert_eq!(ctx("Sub\n-"), [h(2), u(2, 5)]);
        assert_eq!(ctx("x\nTitle  \n   ====  "), [Normal, h(1), u(1, 7)]);
    }

    #[test]
    fn not_setext() {
        // A rule after a blank line, a list item or a heading stays a rule.
        assert_eq!(ctx("\n---"), [Normal, Normal]);
        assert_eq!(ctx("- item\n---"), [Normal, Normal]);
        assert_eq!(ctx("# H\n==="), [Normal, Normal]);
        assert_eq!(ctx("> q\n---")[1], Normal);
        // Underline indented 4+ spaces, or with other characters.
        assert_eq!(ctx("T\n    ==="), [Normal, Normal]);
        assert_eq!(ctx("T\n=-="), [Normal, Normal]);
        // A fence close or frontmatter end isn't heading text.
        assert_eq!(ctx("```\nx\n```\n---")[3], Normal);
        assert_eq!(ctx("---\na: 1\n---")[2], Frontmatter);
    }

    #[test]
    fn setext_pair_is_one_reveal_group() {
        let lines: Vec<String> = ["a", "Title", "===", "b"].map(String::from).to_vec();
        assert_eq!(analyze(&lines).reveal_group(2), Some((1, 2)));
    }

    #[test]
    fn reveal_groups_cover_frontmatter_and_fences() {
        let lines: Vec<String> = "---\na: 1\n---\ntext\n```\ncode\n```\nend"
            .lines()
            .map(String::from)
            .collect();
        let s = analyze(&lines);
        assert_eq!(s.reveal_groups, [(0, 2), (4, 6)]);
        assert_eq!(s.reveal_group(1), Some((0, 2)));
        assert_eq!(s.reveal_group(3), None);
        assert_eq!(s.reveal_group(6), Some((4, 6)));
    }

    #[test]
    fn unclosed_fence_reveal_group_runs_to_end() {
        let lines: Vec<String> = ["a", "```", "b"].map(String::from).to_vec();
        assert_eq!(analyze(&lines).reveal_groups, [(1, 2)]);
    }
}
