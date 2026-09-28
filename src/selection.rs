//! Text selection (V-19): Shift + arrows, Home / End, Page Up / Down, and
//! Ctrl+A. The selection runs from the anchor to the cursor; typing replaces
//! it, a wrap character wraps it (V-05) and a pasted URL links it (V-09).

use crate::editor::Editor;

/// A position in the document: (line, char column).
pub type Pos = (usize, usize);

impl Editor {
    /// The selected range, start before end, or `None` if nothing is
    /// selected.
    pub fn selection(&self) -> Option<(Pos, Pos)> {
        let anchor = self.anchor?;
        let cursor = (self.row, self.col);
        (anchor != cursor).then(|| (anchor.min(cursor), anchor.max(cursor)))
    }

    /// Called before the cursor moves: Shift starts (or keeps) a selection
    /// at the cursor, a plain move ends it.
    pub fn select_with(&mut self, extend: bool) {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some((self.row, self.col));
        }
    }

    /// Selects the whole document (Ctrl+A).
    pub fn select_all(&mut self) {
        self.anchor = Some((0, 0));
        self.row = self.lines.len() - 1;
        self.col = self.lines[self.row].chars().count();
    }

    /// The selected text, lines joined with `\n`.
    pub fn selected_text(&self) -> Option<String> {
        let ((r0, c0), (r1, c1)) = self.selection()?;
        let text = (r0..=r1)
            .map(|r| {
                let line = &self.lines[r];
                let from = if r == r0 { byte_at(line, c0) } else { 0 };
                let to = if r == r1 {
                    byte_at(line, c1)
                } else {
                    line.len()
                };
                &line[from..to]
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(text)
    }

    /// Deletes the selected text and puts the cursor where it was. Returns
    /// false if nothing was selected.
    pub fn delete_selection(&mut self) -> bool {
        let Some(((r0, c0), (r1, c1))) = self.selection() else {
            self.anchor = None;
            return false;
        };
        let tail = self.lines[r1][byte_at(&self.lines[r1], c1)..].to_string();
        let first = &mut self.lines[r0];
        first.truncate(byte_at(first, c0));
        first.push_str(&tail);
        self.lines.drain(r0 + 1..=r1);
        (self.row, self.col) = (r0, c0);
        self.anchor = None;
        self.dirty = true;
        true
    }

    /// Puts `open` before and `close` after the selection (V-05) and keeps
    /// the same text selected, so `*` twice makes it bold. Returns false if
    /// nothing was selected.
    pub fn wrap_selection(&mut self, open: &str, close: &str) -> bool {
        let Some(((r0, c0), (r1, c1))) = self.selection() else {
            return false;
        };
        let end = byte_at(&self.lines[r1], c1);
        self.lines[r1].insert_str(end, close);
        let start = byte_at(&self.lines[r0], c0);
        self.lines[r0].insert_str(start, open);
        let shift = open.chars().count();
        let c1 = if r0 == r1 { c1 + shift } else { c1 };
        self.anchor = Some((r0, c0 + shift));
        (self.row, self.col) = (r1, c1);
        self.dirty = true;
        true
    }
}

/// Byte index of char column `col` in `line`.
fn byte_at(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(b, _)| b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(text: &str, at: Pos) -> Editor {
        let mut e = Editor::from_text(text);
        (e.row, e.col) = at;
        e
    }

    /// Selects from the cursor to `to`, as Shift + arrows would.
    fn select_to(e: &mut Editor, to: Pos) {
        e.select_with(true);
        (e.row, e.col) = to;
    }

    #[test]
    fn a_selection_is_ordered_and_ends_with_a_plain_move() {
        let mut e = ed("one two\nthree", (1, 2));
        select_to(&mut e, (0, 4));
        assert_eq!(e.selection(), Some(((0, 4), (1, 2))));
        assert_eq!(e.selected_text().as_deref(), Some("two\nth"));
        e.select_with(true);
        assert_eq!(e.selection(), Some(((0, 4), (1, 2))), "Shift keeps it");
        e.select_with(false);
        assert_eq!(e.selection(), None);
    }

    #[test]
    fn an_empty_selection_is_none() {
        let mut e = ed("abc", (0, 1));
        e.select_with(true);
        assert_eq!(e.selection(), None);
        assert!(!e.delete_selection());
    }

    #[test]
    fn delete_selection_within_and_across_lines() {
        let mut e = ed("one two three", (0, 4));
        select_to(&mut e, (0, 8));
        assert!(e.delete_selection());
        assert_eq!(e.lines, ["one three"]);
        assert_eq!((e.row, e.col), (0, 4));
        assert!(e.dirty);

        let mut e = ed("aé1\nmiddle\n2bc", (2, 1));
        select_to(&mut e, (0, 2));
        assert!(e.delete_selection());
        assert_eq!(e.lines, ["aébc"]);
        assert_eq!((e.row, e.col), (0, 2));
        assert_eq!(e.selection(), None);
    }

    #[test]
    fn select_all_covers_the_document() {
        let mut e = ed("a\nbc", (0, 0));
        e.select_all();
        assert_eq!(e.selection(), Some(((0, 0), (1, 2))));
    }

    #[test]
    fn wrapping_keeps_the_text_selected() {
        let mut e = ed("a word here", (0, 2));
        select_to(&mut e, (0, 6));
        assert!(e.wrap_selection("*", "*"));
        assert!(e.wrap_selection("*", "*"));
        assert_eq!(e.lines, ["a **word** here"]);
        assert_eq!(e.selection(), Some(((0, 4), (0, 8))));
        assert_eq!(e.selected_text().as_deref(), Some("word"));
    }

    #[test]
    fn wrapping_across_lines_wraps_the_ends() {
        let mut e = ed("one\ntwo", (1, 3));
        select_to(&mut e, (0, 0));
        assert!(e.wrap_selection("==", "=="));
        assert_eq!(e.lines, ["==one", "two=="]);
        assert_eq!(e.selected_text().as_deref(), Some("one\ntwo"));
    }
}
