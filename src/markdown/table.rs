//! Tables (B-11 … B-13): cells, the separator row, alignment, widths and
//! borders.

use super::*;

/// The cells of a table row: outer pipes are optional, cells are trimmed,
/// and an escaped `\|` stays inside its cell (e.g. `[[Note\|alias]]`, B-13).
pub fn table_cells(line: &str) -> Vec<&str> {
    let row = line.trim();
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = match row.strip_suffix('|') {
        Some(inner) if !inner.ends_with('\\') => inner,
        _ => row,
    };
    let mut cells = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (i, c) in row.char_indices() {
        match c {
            '\\' => escaped = !escaped,
            '|' if !escaped => {
                cells.push(row[start..i].trim());
                start = i + 1;
            }
            _ => escaped = false,
        }
    }
    cells.push(row[start..].trim());
    cells
}

/// The number of columns if `line` is a table separator row (`|---|:--:|`).
pub fn table_separator(line: &str) -> Option<usize> {
    let cells = table_cells(line);
    let is_rule = |c: &&str| {
        let c = c.strip_prefix(':').unwrap_or(c);
        let c = c.strip_suffix(':').unwrap_or(c);
        !c.is_empty() && c.chars().all(|x| x == '-')
    };
    (line.contains('|') && cells.iter().all(is_rule)).then_some(cells.len())
}

/// Column alignment of a table (B-12), from its separator row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// The column alignments of a separator row: `:--` / `---` left, `:-:`
/// center, `--:` right.
pub fn table_aligns(separator: &str) -> Vec<Align> {
    table_cells(separator)
        .iter()
        .map(|c| match (c.starts_with(':'), c.ends_with(':')) {
            (true, true) => Align::Center,
            (false, true) => Align::Right,
            _ => Align::Left,
        })
        .collect()
}

/// Display width of a table cell as rendered (markup hidden).
pub fn cell_width(cell: &str) -> usize {
    inline_spans(cell, Style::default())
        .iter()
        .map(|s| s.content.width())
        .sum()
}

/// A table border row: `┌──┬──┐`, `├──┼──┤` or `└──┴──┘` for `widths`.
pub fn table_border(widths: &[usize], left: &str, mid: &str, right: &str) -> Line<'static> {
    let inner: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    Line::from(Span::styled(
        format!("{left}{}{right}", inner.join(mid)),
        Style::default().fg(Color::DarkGray),
    ))
}

/// A table header or body row drawn in columns of `widths`, each cell
/// aligned per `aligns` (header cells bold). Missing cells are empty, extra
/// cells are dropped.
pub fn render_table_row(
    line: &str,
    widths: &[usize],
    aligns: &[Align],
    header: bool,
) -> Line<'static> {
    let border = Style::default().fg(Color::DarkGray);
    let base = if header {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let cells = table_cells(line);
    let mut spans = vec![Span::styled("│", border)];
    for (col, &width) in widths.iter().enumerate() {
        let cell = cells.get(col).copied().unwrap_or("");
        let free = width.saturating_sub(cell_width(cell));
        let before = match aligns.get(col).copied().unwrap_or_default() {
            Align::Left => 0,
            Align::Center => free / 2,
            Align::Right => free,
        };
        spans.push(Span::raw(" ".repeat(before + 1)));
        spans.extend(inline_spans(cell, base));
        spans.push(Span::raw(" ".repeat(free - before + 1)));
        spans.push(Span::styled("│", border));
    }
    Line::from(spans)
}
