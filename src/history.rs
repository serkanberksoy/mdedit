//! Undo / redo (V-18). Each change is stored as the lines it replaced and
//! the lines it put there (only the part of the document that differs), so
//! the history stays small even for a large note. Typing and deleting are
//! grouped into word-sized steps; 1000 steps are kept unless `undo_steps` in
//! `config.toml` says otherwise.

/// What made a change, for grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A typed character; `true` for a space (which starts a new step).
    Typing(bool),
    /// Backspace or Delete.
    Deleting,
    /// Anything else: one step each.
    Other,
}

/// One undoable change: lines `start..start + old.len()` were `old` and
/// became `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Change {
    start: usize,
    old: Vec<String>,
    new: Vec<String>,
    /// Cursor before and after the change.
    before: (usize, usize),
    after: (usize, usize),
    kind: Kind,
}

/// The undo and redo stacks of one document.
#[derive(Debug)]
pub struct History {
    undo: Vec<Change>,
    redo: Vec<Change>,
    /// How many changes were on the undo stack when the file was saved
    /// (`None`: that state can't be reached any more).
    saved: Option<usize>,
    /// How many steps can be undone (`undo_steps` in `config.toml`);
    /// older ones are forgotten.
    pub limit: usize,
}

impl Default for History {
    fn default() -> Self {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
            saved: None,
            limit: Self::DEFAULT_LIMIT,
        }
    }
}

impl History {
    /// Steps that can be undone unless the settings say otherwise.
    pub const DEFAULT_LIMIT: usize = 1000;

    /// Records the change from `before` to `after` (nothing if they're
    /// equal). `cursor` is the cursor before and after it.
    pub fn record(
        &mut self,
        before: &[String],
        after: &[String],
        cursor: ((usize, usize), (usize, usize)),
        kind: Kind,
    ) {
        let prefix = before.iter().zip(after).take_while(|(a, b)| a == b).count();
        let room = before.len().min(after.len()) - prefix;
        let suffix = before
            .iter()
            .rev()
            .zip(after.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        if prefix == before.len() && prefix == after.len() {
            return; // nothing changed
        }
        let change = Change {
            start: prefix,
            old: before[prefix..before.len() - suffix].to_vec(),
            new: after[prefix..after.len() - suffix].to_vec(),
            before: cursor.0,
            after: cursor.1,
            kind,
        };
        self.redo.clear();
        if self.saved.is_some_and(|saved| saved > self.undo.len()) {
            self.saved = None; // the saved state was undone and replaced
        }
        let at_save = self.is_saved();
        if let Some(top) = self.undo.last_mut()
            && !at_save
            && groups_with(top, &change)
        {
            top.new = change.new;
            top.after = change.after;
            return;
        }
        self.undo.push(change);
        while self.undo.len() > self.limit.max(1) {
            self.undo.remove(0);
            self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
        }
    }

    /// Reverts the last change in `lines`; returns where the cursor goes.
    /// Whether there's a change to undo (for a host that undoes
    /// something else when there isn't).
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there's an undone change to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, lines: &mut Vec<String>) -> Option<(usize, usize)> {
        let change = self.undo.pop()?;
        let end = change.start + change.new.len();
        lines.splice(change.start..end, change.old.iter().cloned());
        let cursor = change.before;
        self.redo.push(change);
        Some(cursor)
    }

    /// Applies the last undone change again; returns where the cursor goes.
    pub fn redo(&mut self, lines: &mut Vec<String>) -> Option<(usize, usize)> {
        let change = self.redo.pop()?;
        let end = change.start + change.old.len();
        lines.splice(change.start..end, change.new.iter().cloned());
        let cursor = change.after;
        self.undo.push(change);
        Some(cursor)
    }

    /// Remembers the current state as the saved one.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.undo.len());
    }

    /// Whether the document is back in the state it was saved in.
    pub fn is_saved(&self) -> bool {
        self.saved == Some(self.undo.len())
    }
}

/// Whether `next` continues the step `top`: more typing (a space starts a
/// new word) or more deleting, in the same line, right where `top` ended.
fn groups_with(top: &Change, next: &Change) -> bool {
    let same_kind = match (top.kind, next.kind) {
        (Kind::Typing(_), Kind::Typing(space)) => !space,
        (Kind::Deleting, Kind::Deleting) => true,
        _ => false,
    };
    let one_line = |c: &Change| c.old.len() == 1 && c.new.len() == 1;
    same_kind
        && top.after == next.before
        && top.start == next.start
        && one_line(top)
        && one_line(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Vec<String> {
        text.split('\n').map(String::from).collect()
    }

    /// Applies `to` over `lines` and records it.
    fn change(h: &mut History, lines: &mut Vec<String>, to: &str, kind: Kind) {
        let after = doc(to);
        h.record(lines, &after, ((0, 0), (0, 0)), kind);
        *lines = after;
    }

    #[test]
    fn undo_and_redo_restore_the_text_and_cursor() {
        let mut h = History::default();
        let mut lines = doc("a\nb\nc");
        let after = doc("a\nB\nB2\nc");
        h.record(&lines, &after, ((1, 0), (2, 2)), Kind::Other);
        lines = after;
        assert_eq!(h.undo(&mut lines), Some((1, 0)));
        assert_eq!(lines, doc("a\nb\nc"));
        assert_eq!(h.redo(&mut lines), Some((2, 2)));
        assert_eq!(lines, doc("a\nB\nB2\nc"));
        assert_eq!(h.redo(&mut lines), None, "nothing more to redo");
    }

    #[test]
    fn only_the_changed_lines_are_stored() {
        let mut h = History::default();
        let before: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
        let mut after = before.clone();
        after[500].push('!');
        h.record(&before, &after, ((0, 0), (0, 0)), Kind::Other);
        assert_eq!(h.undo[0].start, 500);
        assert_eq!((h.undo[0].old.len(), h.undo[0].new.len()), (1, 1));
    }

    #[test]
    fn typing_is_grouped_by_word() {
        let mut h = History::default();
        let mut lines = doc("");
        let mut typed = String::new();
        for c in "hello world".chars() {
            typed.push(c);
            let n = typed.chars().count();
            let after = doc(&typed);
            h.record(&lines, &after, ((0, n - 1), (0, n)), Kind::Typing(c == ' '));
            lines = after;
        }
        h.undo(&mut lines);
        assert_eq!(lines, doc("hello"));
        h.undo(&mut lines);
        assert_eq!(lines, doc(""));
    }

    #[test]
    fn a_new_change_clears_redo() {
        let mut h = History::default();
        let mut lines = doc("a");
        change(&mut h, &mut lines, "b", Kind::Other);
        h.undo(&mut lines);
        change(&mut h, &mut lines, "c", Kind::Other);
        assert_eq!(h.redo(&mut lines), None);
        assert_eq!(lines, doc("c"));
    }

    #[test]
    fn saved_state_is_known_after_undo_and_redo() {
        let mut h = History::default();
        let mut lines = doc("a");
        h.mark_saved();
        change(&mut h, &mut lines, "b", Kind::Other);
        assert!(!h.is_saved());
        h.undo(&mut lines);
        assert!(h.is_saved());
        h.redo(&mut lines);
        assert!(!h.is_saved());
    }

    #[test]
    fn typing_right_after_a_save_starts_a_new_step() {
        let mut h = History::default();
        let mut lines = doc("a");
        change(&mut h, &mut lines, "ab", Kind::Typing(false));
        h.mark_saved();
        change(&mut h, &mut lines, "abc", Kind::Typing(false));
        h.undo(&mut lines);
        assert_eq!(lines, doc("ab"));
        assert!(h.is_saved());
    }

    /// How many steps can be undone after `changes` changes.
    fn undoable(mut h: History, changes: usize) -> (usize, Vec<String>) {
        let mut lines = doc("0");
        for i in 1..=changes {
            change(&mut h, &mut lines, &i.to_string(), Kind::Other);
        }
        let mut undone = 0;
        while h.undo(&mut lines).is_some() {
            undone += 1;
        }
        (undone, lines)
    }

    #[test]
    fn it_says_whether_there_is_something_to_undo_or_redo() {
        let mut h = History::default();
        let mut lines = doc("0");
        assert!(!h.can_undo() && !h.can_redo());
        change(&mut h, &mut lines, "1", Kind::Other);
        assert!(h.can_undo() && !h.can_redo());
        h.undo(&mut lines);
        assert!(!h.can_undo() && h.can_redo());
    }

    #[test]
    fn a_thousand_steps_can_be_undone_by_default() {
        let (undone, lines) = undoable(History::default(), 1003);
        assert_eq!(undone, 1000);
        assert_eq!(lines, doc("3"), "the 1000 newest changes are undone");
    }

    #[test]
    fn the_limit_can_be_changed() {
        let h = History {
            limit: 100,
            ..History::default()
        };
        assert_eq!(undoable(h, 120).0, 100);
    }

    #[test]
    fn the_saved_state_is_forgotten_when_it_falls_off() {
        let mut h = History {
            limit: 5,
            ..History::default()
        };
        let mut lines = doc("0");
        h.mark_saved();
        for i in 1..=6 {
            change(&mut h, &mut lines, &i.to_string(), Kind::Other);
        }
        while h.undo(&mut lines).is_some() {}
        assert_eq!(lines, doc("1"));
        assert!(!h.is_saved(), "the saved text can't be reached any more");
    }
}
