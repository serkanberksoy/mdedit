//! Soft wrap: splits a rendered line into screen rows at word boundaries,
//! with a hanging indent so continuation rows line up with the text after a
//! list marker, quote bar, etc. Also maps between positions in the unwrapped
//! line (display offsets) and (row, x) on screen.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// How a screen row is drawn: normal, or as a DEC double-size line (R-16;
/// only in terminals that support it, see [`crate::terminal`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LineAttr {
    #[default]
    Normal,
    /// Top half of a double-height, double-width row (`ESC # 3`).
    DoubleTop,
    /// Bottom half of a double-height, double-width row (`ESC # 4`).
    DoubleBottom,
    /// Double-width row (`ESC # 6`).
    DoubleWide,
}

/// A line wrapped to a width.
#[derive(Debug)]
pub struct Wrapped {
    pub rows: Vec<Line<'static>>,
    /// Display offset (in the unwrapped line) where each row's content starts.
    pub starts: Vec<usize>,
    /// Display width of the unwrapped line.
    pub len: usize,
    /// Columns of padding before the content of continuation rows.
    pub indent: usize,
    /// Blank rows added above the content (heading spacing, R-17).
    pub top: usize,
    /// How each row is drawn (double-size heading rows, R-16); one entry
    /// per row.
    pub attrs: Vec<LineAttr>,
    /// Where an embedded image's pixels go (E-04), drawn over these rows.
    pub image: Option<ImageSlot>,
    /// A code block processor's action for each row (view mode); empty
    /// when there are none.
    pub actions: Vec<Option<String>>,
}

/// The part of a line's rows that an image covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSlot {
    /// The image file.
    pub path: std::path::PathBuf,
    /// First row and column of the image, within the line's rows.
    pub row: usize,
    pub col: u16,
    /// Size in cells.
    pub cols: u16,
    pub rows: u16,
}

/// One grapheme of the unwrapped line.
struct Cell {
    text: String,
    style: Style,
    width: usize,
    /// Display offset in the unwrapped line.
    offset: usize,
}

/// Wraps `line` to `width` columns. Continuation rows are padded with
/// `indent` spaces (capped at half the width). `width == 0` means no wrap.
pub fn wrap(line: &Line<'static>, indent: usize, width: usize) -> Wrapped {
    let mut cells = Vec::new();
    let mut offset = 0;
    for span in &line.spans {
        let style = line.style.patch(span.style);
        for g in span.content.graphemes(true) {
            let w = g.width();
            cells.push(Cell {
                text: g.to_string(),
                style,
                width: w,
                offset,
            });
            offset += w;
        }
    }
    let len = offset;
    let indent = if width == 0 { 0 } else { indent.min(width / 2) };

    // Row boundaries as cell indices. `hidden[r]` marks a row whose last
    // cell is the break space, which isn't drawn.
    let mut starts = vec![0];
    let mut hidden = Vec::new();
    if width > 0 && len > width {
        let mut row_start = 0;
        let mut row_w = 0;
        let mut last_break: Option<usize> = None;
        let mut i = 0;
        while i < cells.len() {
            let c = &cells[i];
            if row_w + c.width > width && i > row_start {
                if c.text == " " {
                    // The overflowing char is a space: break on it.
                    starts.push(i + 1);
                    hidden.push(true);
                    row_start = i + 1;
                    row_w = indent;
                    last_break = None;
                    i += 1;
                    continue;
                }
                let brk = last_break.filter(|&b| b > row_start).unwrap_or(i);
                // Soft break: the row ends with the space it broke at.
                hidden.push(cells[brk - 1].text == " ");
                starts.push(brk);
                row_start = brk;
                row_w = indent + cells[brk..i].iter().map(|c| c.width).sum::<usize>();
                last_break = None;
            }
            row_w += c.width;
            // Break opportunities: after a space, but never inside the
            // marker prefix of the first row.
            if c.text == " " && c.offset >= indent {
                last_break = Some(i + 1);
            }
            i += 1;
        }
    }
    hidden.push(false);

    let mut rows = Vec::with_capacity(starts.len());
    for (r, &start) in starts.iter().enumerate() {
        let mut end = starts.get(r + 1).copied().unwrap_or(cells.len());
        if hidden[r] {
            end -= 1;
        }
        let mut spans: Vec<Span<'static>> = Vec::new();
        if r > 0 && indent > 0 {
            spans.push(Span::raw(" ".repeat(indent)));
        }
        // Group cells into spans by style (never merging into the padding).
        let content_from = spans.len();
        for c in &cells[start..end] {
            let mergeable = spans.len() > content_from;
            match spans.last_mut() {
                Some(last) if mergeable && last.style == c.style => {
                    last.content.to_mut().push_str(&c.text);
                }
                _ => spans.push(Span::styled(c.text.clone(), c.style)),
            }
        }
        rows.push(Line::from(spans));
    }

    Wrapped {
        rows,
        starts: starts
            .iter()
            .map(|&i| cells.get(i).map_or(len, |c| c.offset))
            .collect(),
        attrs: vec![LineAttr::Normal; starts.len()],
        len,
        indent,
        top: 0,
        image: None,
        actions: Vec::new(),
    }
}

impl Wrapped {
    /// A line that takes no screen rows (hidden in a collapsed fold).
    pub fn empty() -> Self {
        Wrapped {
            rows: Vec::new(),
            starts: vec![0],
            len: 0,
            indent: 0,
            top: 0,
            attrs: Vec::new(),
            image: None,
            actions: Vec::new(),
        }
    }

    /// Appends a normal row (e.g. part of an embed's frame).
    pub fn push_line(&mut self, line: Line<'static>) {
        self.push_row(line, LineAttr::Normal);
    }

    /// Appends a row drawn with `attr` (e.g. an embed's frame).
    pub fn push_row(&mut self, line: Line<'static>, attr: LineAttr) {
        self.rows.push(line);
        self.attrs.push(attr);
    }

    /// Adds `line` as an extra row above the content (e.g. a table border).
    pub fn pad_top_with(&mut self, line: Line<'static>) {
        self.rows.insert(0, line);
        self.attrs.insert(0, LineAttr::Normal);
        self.top += 1;
    }

    /// Adds `line` as an extra row below the content.
    pub fn pad_bottom_with(&mut self, line: Line<'static>) {
        self.push_row(line, LineAttr::Normal);
    }

    /// Adds `n` blank rows above the content.
    pub fn pad_top(&mut self, n: usize) {
        for _ in 0..n {
            self.pad_top_with(Line::default());
        }
    }

    /// Draws the rows as a larger heading (R-16): level 1 doubles every row
    /// (top and bottom halves of double-size text), level 2 makes each row
    /// double-width. The rows must already be wrapped to half the width.
    pub fn enlarge(&mut self, level: u8) {
        if level == 1 {
            let rows = std::mem::take(&mut self.rows);
            self.attrs.clear();
            for row in rows {
                self.rows.push(row.clone());
                self.attrs.push(LineAttr::DoubleTop);
                self.rows.push(row);
                self.attrs.push(LineAttr::DoubleBottom);
            }
        } else {
            self.attrs = vec![LineAttr::DoubleWide; self.rows.len()];
        }
    }
}

impl Wrapped {
    /// Screen (row, x) of the display offset `offset` in the unwrapped line.
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let row = self.starts.iter().rposition(|&s| s <= offset).unwrap_or(0);
        let pad = if row > 0 { self.indent } else { 0 };
        (self.top + row, offset - self.starts[row] + pad)
    }

    /// Display offset at screen column `x` of `row`, clamped to that row.
    pub fn offset_at(&self, row: usize, x: usize) -> usize {
        let row = row.min(self.starts.len() - 1);
        let pad = if row > 0 { self.indent } else { 0 };
        let start = self.starts[row];
        let max = match self.starts.get(row + 1) {
            Some(&next) => next.saturating_sub(1).max(start),
            None => self.len,
        };
        (start + x.saturating_sub(pad)).min(max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    fn rows(w: &Wrapped) -> Vec<String> {
        w.rows
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    fn plain(text: &str) -> Line<'static> {
        Line::from(text.to_string())
    }

    #[test]
    fn short_line_is_one_row() {
        let w = wrap(&plain("hello"), 0, 10);
        assert_eq!(rows(&w), ["hello"]);
        assert_eq!(w.starts, [0]);
        assert_eq!(w.len, 5);
    }

    #[test]
    fn zero_width_means_no_wrap() {
        assert_eq!(rows(&wrap(&plain("a b c d e f"), 0, 0)), ["a b c d e f"]);
    }

    #[test]
    fn breaks_at_last_space_and_hides_it() {
        let w = wrap(&plain("aaa bbb ccc"), 0, 7);
        assert_eq!(rows(&w), ["aaa bbb", "ccc"]);
        assert_eq!(w.starts, [0, 8]);
    }

    #[test]
    fn space_that_exactly_fits_is_still_hidden() {
        // "aaaa bbbb" is 9 wide; the space fits at column 10, "c" doesn't.
        let w = wrap(&plain("aaaa bbbb cccc"), 0, 10);
        assert_eq!(rows(&w), ["aaaa bbbb", "cccc"]);
    }

    #[test]
    fn breaks_inside_row_when_word_does_not_fit() {
        assert_eq!(rows(&wrap(&plain("aa bbbb"), 0, 5)), ["aa", "bbbb"]);
    }

    #[test]
    fn hanging_indent_and_no_break_inside_prefix() {
        let line = Line::from(vec![Span::raw("• "), Span::raw("one two three")]);
        let w = wrap(&line, 2, 9);
        assert_eq!(rows(&w), ["• one two", "  three"]);
        assert_eq!(w.starts, [0, 10]);
    }

    #[test]
    fn long_word_is_hard_broken() {
        let line = Line::from(vec![Span::raw("• "), Span::raw("abcdefghij")]);
        assert_eq!(rows(&wrap(&line, 2, 6)), ["• abcd", "  efgh", "  ij"]);
    }

    #[test]
    fn wide_characters_are_not_split() {
        assert_eq!(rows(&wrap(&plain("ab😀cd"), 0, 3)), ["ab", "😀c", "d"]);
    }

    #[test]
    fn styles_survive_the_break() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let line = Line::from(vec![Span::styled("bold word", bold), Span::raw(" tail")]);
        let w = wrap(&line, 0, 9);
        assert_eq!(rows(&w), ["bold word", "tail"]);
        assert_eq!(w.rows[0].spans[0].style, bold);
        assert_eq!(w.rows[1].spans[0].style, Style::default());
    }

    #[test]
    fn indent_is_capped_at_half_width() {
        let w = wrap(&plain("aaaa bbbb"), 8, 6);
        assert_eq!(rows(&w), ["aaaa", "   bbb", "   b"]);
    }

    #[test]
    fn position_maps_offsets_to_rows() {
        let line = Line::from(vec![Span::raw("• "), Span::raw("one two three")]);
        let w = wrap(&line, 2, 9);
        assert_eq!(w.position(0), (0, 0));
        assert_eq!(w.position(6), (0, 6));
        assert_eq!(w.position(10), (1, 2)); // start of "three"
        assert_eq!(w.position(15), (1, 7)); // end of line
    }

    #[test]
    fn position_of_hidden_break_space_stays_on_its_row() {
        let w = wrap(&plain("aaa bbb ccc"), 0, 7);
        assert_eq!(w.position(7), (0, 7));
    }

    #[test]
    fn offset_at_clamps_to_the_row() {
        let line = Line::from(vec![Span::raw("• "), Span::raw("one two three")]);
        let w = wrap(&line, 2, 9);
        assert_eq!(w.offset_at(1, 4), 12);
        assert_eq!(w.offset_at(1, 0), 10); // inside the indent
        assert_eq!(w.offset_at(1, 50), 15); // past the end of the last row
        assert_eq!(w.offset_at(0, 50), 9); // at the hidden break space
    }
}
