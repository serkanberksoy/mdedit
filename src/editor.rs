//! Source buffer and cursor. Knows nothing about rendering.

use unicode_segmentation::UnicodeSegmentation;

use std::sync::atomic::{AtomicU64, Ordering};

use crate::history::History;
use crate::markdown::{Block, is_done, is_ordered, nesting, next_marker, parse_line};

#[derive(Debug, Default)]
pub struct Editor {
    pub lines: Vec<String>,
    /// Cursor row (line index).
    pub row: usize,
    /// Cursor column, in chars (not bytes). Always on a grapheme boundary,
    /// so the cursor never sits inside a joined emoji or accented letter.
    pub col: usize,
    pub dirty: bool,
    /// The file's line ending, kept on save (F-07).
    pub crlf: bool,
    /// Whether the file ends with a newline, kept on save (F-07).
    pub final_newline: bool,
    /// Undo / redo (V-18).
    pub history: History,
    /// Tells documents apart, so a change is never recorded across a
    /// newly opened file.
    pub id: u64,
    /// Where the selection started (V-19); it runs to the cursor.
    pub anchor: Option<(usize, usize)>,
}

fn byte_idx(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

/// Whether `line` is a list item (bullet, ordered or task).
fn is_item(line: &str) -> bool {
    matches!(parse_line(line), Block::Bullet { .. } | Block::Task { .. })
}

/// Whether `line` belongs to a list: an item, or indented continuation text.
fn in_list(line: &str) -> bool {
    is_item(line) || (line.starts_with([' ', '\t']) && !line.trim().is_empty())
}

/// The line range of the list containing `row`, if it's in one. Blank lines
/// between list lines belong to the list.
fn list_run(lines: &[String], row: usize) -> Option<(usize, usize)> {
    let blank = |r: usize| lines[r].trim().is_empty();
    let part = |r: usize| {
        in_list(&lines[r])
            || (blank(r)
                && (0..r)
                    .rev()
                    .find(|&k| !blank(k))
                    .is_some_and(|k| in_list(&lines[k]))
                && (r + 1..lines.len())
                    .find(|&k| !blank(k))
                    .is_some_and(|k| in_list(&lines[k])))
    };
    if !part(row) {
        return None;
    }
    let mut start = row;
    while start > 0 && part(start - 1) {
        start -= 1;
    }
    let mut end = row;
    while end + 1 < lines.len() && part(end + 1) {
        end += 1;
    }
    Some((start, end))
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Char offsets where grapheme clusters start, plus the end of the string.
fn grapheme_starts(s: &str) -> impl Iterator<Item = usize> + '_ {
    s.graphemes(true)
        .scan(0, |col, g| {
            let start = *col;
            *col += g.chars().count();
            Some(start)
        })
        .chain(std::iter::once(char_len(s)))
}

/// The grapheme boundary before `col` (0 if there is none).
fn prev_boundary(s: &str, col: usize) -> usize {
    grapheme_starts(s)
        .take_while(|&b| b < col)
        .last()
        .unwrap_or(0)
}

/// The grapheme boundary after `col` (the end if there is none).
fn next_boundary(s: &str, col: usize) -> usize {
    grapheme_starts(s)
        .find(|&b| b > col)
        .unwrap_or_else(|| char_len(s))
}

/// `col` moved back to the start of the grapheme it falls in.
fn snap_to_boundary(s: &str, col: usize) -> usize {
    grapheme_starts(s)
        .take_while(|&b| b <= col)
        .last()
        .unwrap_or(0)
}

impl Editor {
    /// A document from a file's text. Its line ending (the first line's)
    /// and final newline are remembered for [`Editor::to_text`]; an empty
    /// text (a new file) gets LF and a final newline.
    pub fn from_text(text: &str) -> Self {
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let mut history = History::default();
        history.mark_saved();
        Self {
            lines,
            history,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            crlf: text.find('\n').is_some_and(|i| text[..i].ends_with('\r')),
            final_newline: text.is_empty() || text.ends_with('\n'),
            ..Default::default()
        }
    }

    /// Undoes (or redoes) the last change and moves the cursor there.
    /// Returns false if there was nothing to undo (redo).
    pub fn undo(&mut self, redo: bool) -> bool {
        let cursor = if redo {
            self.history.redo(&mut self.lines)
        } else {
            self.history.undo(&mut self.lines)
        };
        let Some((row, col)) = cursor else {
            return false;
        };
        self.row = row.min(self.lines.len() - 1);
        self.col = col;
        self.clamp_col();
        self.dirty = !self.history.is_saved();
        true
    }

    /// The document was just saved.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.history.mark_saved();
    }

    /// The text to save, with the file's own line endings.
    pub fn to_text(&self) -> String {
        let ending = if self.crlf { "\r\n" } else { "\n" };
        let mut s = self.lines.join(ending);
        if self.final_newline {
            s.push_str(ending);
        }
        s
    }

    fn current(&self) -> &String {
        &self.lines[self.row]
    }

    /// Moves the cursor back to the start of the grapheme it's in.
    pub fn snap_col(&mut self) {
        self.clamp_col();
    }

    fn clamp_col(&mut self) {
        self.col = snap_to_boundary(self.current(), self.col);
    }

    pub fn insert_char(&mut self, c: char) {
        let row = self.row;
        let at = byte_idx(&self.lines[row], self.col);
        self.lines[row].insert(at, c);
        self.col += 1;
        self.dirty = true;
    }

    /// Inserts `text` (no newlines) at the cursor and moves past it.
    pub fn insert_str(&mut self, text: &str) {
        let row = self.row;
        let at = byte_idx(&self.lines[row], self.col);
        self.lines[row].insert_str(at, text);
        self.col += char_len(text);
        self.dirty = true;
    }

    /// Inserts `text` (which may span lines) at the cursor exactly as it
    /// is, and moves past it.
    pub fn paste(&mut self, text: &str) {
        let mut pieces = text.split('\n');
        let first = pieces.next().unwrap_or_default();
        let rest: Vec<&str> = pieces.collect();
        let Some((&last, middle)) = rest.split_last() else {
            self.insert_str(first);
            return;
        };
        let line = &mut self.lines[self.row];
        let after = line.split_off(byte_idx(line, self.col));
        line.push_str(first);
        let mut added: Vec<String> = middle.iter().map(|l| l.to_string()).collect();
        added.push(format!("{last}{after}"));
        let at = self.row + 1;
        self.lines.splice(at..at, added);
        self.row += rest.len();
        self.col = char_len(last);
        self.dirty = true;
    }

    /// Renumbers the ordered list around the cursor (L-08). Each nesting
    /// level counts on from its first item's number; a bullet at the same
    /// level or a different delimiter (`.` / `)`) starts a new sequence.
    pub fn renumber(&mut self) {
        let Some((start, end)) = list_run(&self.lines, self.row) else {
            return;
        };
        // Open sequences, outermost first: (level, delimiter, next number).
        let mut open: Vec<(usize, char, u64)> = Vec::new();
        for r in start..=end {
            let (indent, marker) = match parse_line(&self.lines[r]) {
                Block::Bullet { indent, marker, .. } | Block::Task { indent, marker, .. } => {
                    (indent, marker.to_string())
                }
                _ => continue, // continuation text or a blank line
            };
            let level = nesting(&self.lines[r][..indent]);
            let continues = open.iter().any(|&(l, _, _)| l == level);
            open.retain(|&(l, _, _)| l <= level);
            if !is_ordered(&marker) {
                open.retain(|&(l, _, _)| l < level);
                continue;
            }
            let (digits, delim) = marker.split_at(marker.len() - 1);
            let delim = delim.chars().next().expect("ordered markers end in . or )");
            let own: u64 = digits.parse().unwrap_or(1);
            let number = match open.last() {
                Some(&(l, d, next)) if continues && l == level && d == delim => next,
                _ => own,
            };
            open.retain(|&(l, _, _)| l < level);
            open.push((level, delim, number + 1));
            if number != own {
                let new = format!("{number}{delim}");
                if r == self.row && self.col > indent {
                    self.col = (self.col + new.len()).saturating_sub(marker.len());
                }
                self.lines[r].replace_range(indent..indent + marker.len(), &new);
            }
        }
    }

    /// Splits the line at the cursor. On a bullet or task line the list
    /// continues (new tasks start unchecked); pressing Enter on an empty item
    /// ends the list instead.
    pub fn newline(&mut self) {
        self.dirty = true;
        let line = self.current().clone();

        let list = match parse_line(&line) {
            Block::Bullet {
                indent,
                marker,
                text,
            } => Some((
                format!("{}{} ", &line[..indent], next_marker(marker)),
                indent + marker.len() + 1,
                text,
            )),
            Block::Task {
                indent,
                marker,
                state,
                text,
            } => {
                // V-14: the new task keeps this one's type (`[.]` log,
                // `[/]` in progress …); after a finished one it's open.
                let next = if is_done(state) || state == '-' {
                    ' '
                } else {
                    state
                };
                Some((
                    format!("{}{} [{next}] ", &line[..indent], next_marker(marker)),
                    indent + marker.len() + 4 + state.len_utf8(),
                    text,
                ))
            }
            _ => None,
        };
        if let Some((prefix, existing_len, text)) = list {
            if text.is_empty() {
                self.lines[self.row].clear();
                self.col = 0;
                return;
            }
            if self.col >= existing_len {
                let at = byte_idx(&line, self.col);
                self.lines[self.row] = line[..at].to_string();
                self.lines
                    .insert(self.row + 1, format!("{prefix}{}", &line[at..]));
                self.row += 1;
                self.col = char_len(&prefix);
                self.renumber();
                return;
            }
        }

        let at = byte_idx(&line, self.col);
        self.lines[self.row] = line[..at].to_string();
        self.lines.insert(self.row + 1, line[at..].to_string());
        self.row += 1;
        self.col = 0;
        self.renumber();
    }

    /// Turns the current line into an open task, keeping its text:
    /// a bullet gains a checkbox, plain text gains `- [ ] `. On a line that is
    /// already a task, or a heading, a new empty task is opened below instead.
    pub fn create_task(&mut self) {
        let row = self.row;
        let line = self.current().clone();
        match parse_line(&line) {
            Block::Bullet { indent, marker, .. } => {
                let at = (indent + marker.len() + 1).min(line.len());
                self.lines[row].insert_str(at, "[ ] ");
                if self.col >= at {
                    self.col += 4;
                }
            }
            Block::Paragraph(_) => {
                let indent = line.chars().take_while(|&c| c == ' ' || c == '\t').count();
                self.lines[row].insert_str(indent, "- [ ] ");
                self.col = if self.col >= indent {
                    self.col + 6
                } else {
                    indent + 6
                };
            }
            Block::Task { indent, marker, .. } => {
                let prefix = format!("{}{} [ ] ", &line[..indent], next_marker(marker));
                self.col = char_len(&prefix);
                self.lines.insert(row + 1, prefix);
                self.row += 1;
            }
            Block::Heading { .. } | Block::Quote { .. } | Block::Rule => {
                self.lines.insert(row + 1, "- [ ] ".to_string());
                self.row += 1;
                self.col = 6;
            }
        }
        self.dirty = true;
    }

    /// Closes the task on the current line (`[ ]`, `[.]`, `[/]`, ... -> `[x]`);
    /// on an already closed task it reopens it. Returns false if the line is
    /// not a task.
    pub fn toggle_task(&mut self) -> bool {
        let Block::Task {
            indent,
            marker,
            state,
            ..
        } = parse_line(self.current())
        else {
            return false;
        };
        let row = self.row;
        let at = indent + marker.len() + 2; // the char inside the brackets
        let new = if is_done(state) { " " } else { "x" };
        self.lines[row].replace_range(at..at + state.len_utf8(), new);
        self.dirty = true;
        true
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let row = self.row;
            let start = prev_boundary(&self.lines[row], self.col);
            let range = byte_idx(&self.lines[row], start)..byte_idx(&self.lines[row], self.col);
            self.lines[row].replace_range(range, "");
            self.col = start;
            self.dirty = true;
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = char_len(self.current());
            self.lines[self.row].push_str(&line);
            self.dirty = true;
            self.renumber();
        }
    }

    pub fn delete(&mut self) {
        let row = self.row;
        if self.col < char_len(&self.lines[row]) {
            let end = next_boundary(&self.lines[row], self.col);
            let range = byte_idx(&self.lines[row], self.col)..byte_idx(&self.lines[row], end);
            self.lines[row].replace_range(range, "");
            self.dirty = true;
        } else if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
            self.dirty = true;
            self.renumber();
        }
    }

    /// Tab on a bullet or task line nests it one level deeper, using a tab if
    /// the line is already tab-indented and 2 spaces otherwise. Elsewhere it
    /// inserts 2 spaces.
    pub fn indent(&mut self) {
        if let Block::Bullet { indent, .. } | Block::Task { indent, .. } =
            parse_line(self.current())
        {
            let row = self.row;
            let unit = if self.lines[row][..indent].contains('\t') {
                "\t"
            } else {
                "  "
            };
            self.lines[row].insert_str(0, unit);
            self.col += unit.len();
            self.dirty = true;
            self.restart_numbering();
            self.renumber();
        } else {
            self.insert_char(' ');
            self.insert_char(' ');
        }
    }

    /// Shift-Tab on an indented bullet or task line un-nests it one level.
    pub fn outdent(&mut self) {
        if let Block::Bullet { indent, .. } | Block::Task { indent, .. } =
            parse_line(self.current())
        {
            let row = self.row;
            let ws = &self.lines[row][..indent];
            let n = if ws.starts_with('\t') {
                1
            } else {
                ws.chars().take_while(|&c| c == ' ').count().min(2)
            };
            if n > 0 {
                self.lines[row].drain(..n);
                self.col = self.col.saturating_sub(n);
                self.dirty = true;
                self.renumber();
            }
        }
    }

    /// Sets the current ordered item's number to 1 (it just moved into a
    /// nested list; `renumber` continues it from a sibling, if any).
    fn restart_numbering(&mut self) {
        let row = self.row;
        if let Block::Bullet { indent, marker, .. } | Block::Task { indent, marker, .. } =
            parse_line(&self.lines[row])
            && is_ordered(marker)
        {
            let new = format!("1{}", &marker[marker.len() - 1..]);
            let old_len = marker.len();
            if self.col > indent {
                self.col = (self.col + new.len()).saturating_sub(old_len);
            }
            self.lines[row].replace_range(indent..indent + old_len, &new);
        }
    }

    pub fn left(&mut self) {
        if self.col > 0 {
            self.col = prev_boundary(self.current(), self.col);
        } else if self.row > 0 {
            self.row -= 1;
            self.col = char_len(self.current());
        }
    }

    pub fn right(&mut self) {
        if self.col < char_len(self.current()) {
            self.col = next_boundary(self.current(), self.col);
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.clamp_col();
        }
    }

    pub fn down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.clamp_col();
        }
    }

    pub fn home(&mut self) {
        self.col = 0;
    }

    pub fn end(&mut self) {
        self.col = char_len(self.current());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(ed: &mut Editor, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                ed.newline();
            } else {
                ed.insert_char(c);
            }
        }
    }

    const ZWJ_EMOJI: &str = "🧑\u{200d}🦳"; // 3 chars, 1 grapheme

    #[test]
    fn right_and_left_move_over_whole_graphemes() {
        let mut ed = Editor::from_text(&format!("a{ZWJ_EMOJI}b"));
        ed.col = 1;
        ed.right();
        assert_eq!(ed.col, 4);
        ed.left();
        assert_eq!(ed.col, 1);
    }

    #[test]
    fn combining_accent_is_one_step() {
        let mut ed = Editor::from_text("e\u{301}x");
        ed.right();
        assert_eq!(ed.col, 2);
    }

    #[test]
    fn backspace_and_delete_remove_whole_graphemes() {
        let mut ed = Editor::from_text(&format!("a{ZWJ_EMOJI}b"));
        ed.col = 4;
        ed.backspace();
        assert_eq!((ed.lines[0].as_str(), ed.col), ("ab", 1));

        let mut ed = Editor::from_text(&format!("a{ZWJ_EMOJI}b"));
        ed.col = 1;
        ed.delete();
        assert_eq!((ed.lines[0].as_str(), ed.col), ("ab", 1));
    }

    #[test]
    fn vertical_move_snaps_to_grapheme_start() {
        let mut ed = Editor::from_text(&format!("abcd\na{ZWJ_EMOJI}b"));
        ed.col = 3; // would land inside the emoji on the next line
        ed.down();
        assert_eq!(ed.col, 1);
    }

    #[test]
    fn typing_after_moving_over_emoji_keeps_it_intact() {
        let mut ed = Editor::from_text(ZWJ_EMOJI);
        ed.right();
        ed.insert_char('!');
        assert_eq!(ed.lines[0], format!("{ZWJ_EMOJI}!"));
    }

    #[test]
    fn insert_str_puts_text_at_cursor_and_moves_past_it() {
        let mut ed = Editor::from_text("ab");
        ed.col = 1;
        ed.insert_str("😀x");
        assert_eq!(ed.lines[0], "a😀xb");
        assert_eq!(ed.col, 3);
        assert!(ed.dirty);
    }

    #[test]
    fn typing_and_newlines() {
        let mut ed = Editor::from_text("");
        type_str(&mut ed, "# Title\nbody");
        assert_eq!(ed.lines, ["# Title", "body"]);
        assert_eq!((ed.row, ed.col), (1, 4));
        assert!(ed.dirty);
    }

    #[test]
    fn bullet_continues_and_ends() {
        let mut ed = Editor::from_text("");
        type_str(&mut ed, "- one\ntwo\n\nafter");
        assert_eq!(ed.lines, ["- one", "- two", "after"]);
    }

    #[test]
    fn nested_bullet_continues_with_indent() {
        let mut ed = Editor::from_text("");
        type_str(&mut ed, "* a\n");
        ed.indent();
        type_str(&mut ed, "b\nc");
        assert_eq!(ed.lines, ["* a", "  * b", "  * c"]);
        ed.outdent();
        assert_eq!(ed.lines[2], "* c");
        assert_eq!(ed.col, 3);
    }

    #[test]
    fn ordered_list_continues_with_the_next_number() {
        let mut ed = Editor::from_text("");
        type_str(&mut ed, "1. one\ntwo\n\nafter");
        assert_eq!(ed.lines, ["1. one", "2. two", "after"]);
        let mut ed = Editor::from_text("9) nine");
        ed.end();
        type_str(&mut ed, "\nten");
        assert_eq!(ed.lines, ["9) nine", "10) ten"]);
    }

    #[test]
    fn ordered_tasks_toggle_continue_and_create() {
        let mut ed = Editor::from_text("10. [ ] a");
        assert!(ed.toggle_task());
        assert_eq!(ed.lines[0], "10. [x] a");
        ed.end();
        type_str(&mut ed, "\nb");
        assert_eq!(ed.lines[1], "11. [ ] b");
        let mut ed = Editor::from_text("1. item");
        ed.create_task();
        assert_eq!(ed.lines[0], "1. [ ] item");
    }

    fn renumbered(text: &str, row: usize) -> Vec<String> {
        let mut ed = Editor::from_text(text);
        ed.row = row;
        ed.renumber();
        ed.lines
    }

    #[test]
    fn renumber_counts_from_the_first_item() {
        assert_eq!(renumbered("1. a\n5. b\n9. c", 1), ["1. a", "2. b", "3. c"]);
        assert_eq!(renumbered("3) a\n3) b", 0), ["3) a", "4) b"]);
    }

    #[test]
    fn renumber_levels_separately_and_across_blank_lines() {
        assert_eq!(
            renumbered("1. a\n  1. x\n  5. y\n7. b\n\n9. c", 0),
            ["1. a", "  1. x", "  2. y", "2. b", "", "3. c"]
        );
    }

    #[test]
    fn bullet_or_other_delimiter_starts_a_new_sequence() {
        assert_eq!(
            renumbered("1. a\n- b\n5. c\n7. d", 0),
            ["1. a", "- b", "5. c", "6. d"]
        );
        assert_eq!(renumbered("1. a\n4) b\n9) c", 0), ["1. a", "4) b", "5) c"]);
    }

    #[test]
    fn renumber_stops_at_text_outside_the_list() {
        assert_eq!(
            renumbered("1. a\nplain\n5. b", 0),
            ["1. a", "plain", "5. b"]
        );
    }

    #[test]
    fn enter_in_the_middle_renumbers_the_rest() {
        let mut ed = Editor::from_text("1. one\n2. two\n3. three");
        ed.row = 1;
        ed.end();
        type_str(&mut ed, "\nnew");
        assert_eq!(ed.lines, ["1. one", "2. two", "3. new", "4. three"]);
    }

    #[test]
    fn indenting_an_item_starts_a_nested_list_and_closes_the_gap() {
        let mut ed = Editor::from_text("1. a\n2. b\n3. c");
        ed.row = 1;
        ed.indent();
        assert_eq!(ed.lines, ["1. a", "  1. b", "2. c"]);
        ed.outdent();
        assert_eq!(ed.lines, ["1. a", "2. b", "3. c"]);
    }

    #[test]
    fn cursor_follows_a_longer_number() {
        let mut ed = Editor::from_text("8. a\n9. b\n9. c");
        ed.row = 2;
        ed.col = 4; // after "9. c"
        ed.renumber();
        assert_eq!(ed.lines[2], "10. c");
        assert_eq!(ed.col, 5);
    }

    #[test]
    fn deleting_a_line_break_renumbers() {
        let mut ed = Editor::from_text("1. a\n2. b\n\n3. c");
        ed.row = 2;
        ed.backspace(); // removes the empty line: still one list
        assert_eq!(ed.lines, ["1. a", "2. b", "3. c"]);
        ed.row = 1;
        ed.home();
        ed.backspace(); // joins "2. b" into "1. a"
        assert_eq!(ed.lines, ["1. a2. b", "2. c"]);
    }

    #[test]
    fn task_type_continues_on_enter() {
        for state in ['.', '/', '>', '!', '?', ' '] {
            let mut ed = Editor::from_text(&format!("- [{state}] one"));
            ed.end();
            type_str(&mut ed, "\ntwo");
            assert_eq!(ed.lines[1], format!("- [{state}] two"), "state {state:?}");
        }
    }

    #[test]
    fn finished_tasks_continue_as_open_tasks() {
        for state in ['x', 'X', '-'] {
            let mut ed = Editor::from_text(&format!("1. [{state}] one"));
            ed.end();
            type_str(&mut ed, "\ntwo");
            assert_eq!(ed.lines[1], "2. [ ] two", "state {state:?}");
        }
    }

    #[test]
    fn task_continues_unchecked_and_ends() {
        let mut ed = Editor::from_text("- [x] done");
        ed.end();
        type_str(&mut ed, "\nnext\n\nafter");
        assert_eq!(ed.lines, ["- [x] done", "- [ ] next", "after"]);
    }

    #[test]
    fn create_task_from_each_block_kind() {
        let mut ed = Editor::from_text("");
        ed.create_task();
        type_str(&mut ed, "a");
        assert_eq!(ed.lines, ["- [ ] a"]);

        let mut ed = Editor::from_text("buy milk");
        ed.col = 3;
        ed.create_task();
        assert_eq!(ed.lines, ["- [ ] buy milk"]);
        assert_eq!(ed.col, 9);

        let mut ed = Editor::from_text("  * item");
        ed.end();
        ed.create_task();
        assert_eq!(ed.lines, ["  * [ ] item"]);
        assert_eq!(ed.col, 12);

        let mut ed = Editor::from_text("  - [x] old");
        ed.create_task();
        type_str(&mut ed, "new");
        assert_eq!(ed.lines, ["  - [x] old", "  - [ ] new"]);

        let mut ed = Editor::from_text("# Todo");
        ed.create_task();
        assert_eq!(ed.lines, ["# Todo", "- [ ] "]);
        assert_eq!((ed.row, ed.col), (1, 6));
    }

    #[test]
    fn toggle_task_closes_and_reopens() {
        let mut ed = Editor::from_text("  - [ ] thing\nplain");
        assert!(ed.toggle_task());
        assert_eq!(ed.lines[0], "  - [x] thing");
        assert!(ed.toggle_task());
        assert_eq!(ed.lines[0], "  - [ ] thing");
        ed.down();
        assert!(!ed.toggle_task());
        assert_eq!(ed.lines[1], "plain");
    }

    #[test]
    fn tab_indented_lists() {
        let mut ed = Editor::from_text("- 10:00\n\t- [.] log");
        ed.row = 1;
        ed.end();
        type_str(&mut ed, "\nmore");
        assert_eq!(ed.lines[2], "\t- [.] more");
        ed.indent();
        assert_eq!(ed.lines[2], "\t\t- [.] more");
        ed.outdent();
        ed.outdent();
        assert_eq!(ed.lines[2], "- [.] more");
        ed.indent();
        assert_eq!(ed.lines[2], "  - [.] more");
    }

    #[test]
    fn toggle_alternate_state_closes() {
        let mut ed = Editor::from_text("\t- [.] log");
        assert!(ed.toggle_task());
        assert_eq!(ed.lines[0], "\t- [x] log");
        assert!(ed.toggle_task());
        assert_eq!(ed.lines[0], "\t- [ ] log");
    }

    #[test]
    fn split_bullet_mid_text() {
        let mut ed = Editor::from_text("- abcd");
        ed.col = 4;
        ed.newline();
        assert_eq!(ed.lines, ["- ab", "- cd"]);
        assert_eq!((ed.row, ed.col), (1, 2));
    }

    #[test]
    fn backspace_and_delete_join_lines() {
        let mut ed = Editor::from_text("ab\ncd");
        ed.row = 1;
        ed.backspace();
        assert_eq!(ed.lines, ["abcd"]);
        assert_eq!(ed.col, 2);
        ed.backspace();
        assert_eq!(ed.lines, ["acd"]);
        ed.end();
        type_str(&mut ed, "\nx");
        ed.up();
        ed.end();
        ed.delete();
        assert_eq!(ed.lines, ["acdx"]);
    }

    #[test]
    fn multibyte_chars() {
        let mut ed = Editor::from_text("héllo");
        ed.col = 2;
        ed.insert_char('ü');
        assert_eq!(ed.lines[0], "héüllo");
        ed.backspace();
        ed.backspace();
        assert_eq!(ed.lines[0], "hllo");
    }

    #[test]
    fn cursor_movement_clamps() {
        let mut ed = Editor::from_text("long line\nab");
        ed.end();
        ed.down();
        assert_eq!((ed.row, ed.col), (1, 2));
        ed.right();
        assert_eq!((ed.row, ed.col), (1, 2));
        ed.home();
        ed.left();
        assert_eq!((ed.row, ed.col), (0, 9));
    }

    #[test]
    fn round_trip_text() {
        let src = "# T\n- a\n  - b\n";
        assert_eq!(Editor::from_text(src).to_text(), src);
    }
}
