//! Auto-pair (V-04): typing an opening bracket or backtick inserts its
//! closer, typing the second `*`, `=` or `~` of `**`, `==`, `~~` closes the
//! span, typing a closer steps over the one already there, and Backspace
//! between an empty pair deletes both. Off with `auto_pair = "off"`.

use crate::editor::Editor;

impl Editor {
    /// Types `c` with auto-pairing.
    pub fn type_char(&mut self, c: char) {
        let chars: Vec<char> = self.lines[self.row].chars().collect();
        let prev = self.col.checked_sub(1).and_then(|i| chars.get(i).copied());
        let before_prev = self.col.checked_sub(2).and_then(|i| chars.get(i).copied());
        let next = chars.get(self.col).copied();
        // At the end of a word or before a space / closer: room for a pair.
        let open_after = next.is_none_or(|n| n.is_whitespace() || matches!(n, ')' | ']' | '}'));
        match c {
            // A third `*` in an opening `**|**` grows both sides: `***|***`.
            '*' | '=' | '~'
                if prev == Some(c)
                    && before_prev == Some(c)
                    && next == Some(c)
                    && self
                        .col
                        .checked_sub(3)
                        .is_none_or(|i| chars.get(i).is_some_and(|c| c.is_whitespace())) =>
            {
                self.insert_pair(c, c);
            }
            // A closer steps over the same closer.
            ')' | ']' | '}' | '`' if next == Some(c) => self.col += 1,
            // `**bold*|*`: the closing delimiter steps over too.
            '*' | '=' | '~' if next == Some(c) && prev.is_some_and(|p| !p.is_whitespace()) => {
                self.col += 1;
            }
            '(' | '[' | '{' if open_after => self.insert_pair(c, closer(c)),
            // Not after a backtick, so ``` can be typed for a fence.
            '`' if open_after && prev != Some('`') => self.insert_pair(c, '`'),
            // The second char of `**`, `==`, `~~` (not a third, `***`).
            '*' | '=' | '~' if open_after && prev == Some(c) && before_prev != Some(c) => {
                self.insert_char(c);
                let at = self.col;
                self.insert_str(&format!("{c}{c}"));
                self.col = at;
            }
            _ => self.insert_char(c),
        }
    }

    /// Inserts `open` and `close` with the cursor between them.
    fn insert_pair(&mut self, open: char, close: char) {
        self.insert_char(open);
        self.insert_char(close);
        self.col -= 1;
    }

    /// Backspace that also deletes the closer of an empty pair (`(|)`).
    pub fn backspace_pair(&mut self) {
        let chars: Vec<char> = self.lines[self.row].chars().collect();
        let prev = self.col.checked_sub(1).and_then(|i| chars.get(i).copied());
        let next = chars.get(self.col).copied();
        if let (Some(p), Some(n)) = (prev, next)
            && closer(p) == n
        {
            self.delete();
        }
        self.backspace();
    }
}

/// The char that closes `open` (itself for delimiters like `*`).
fn closer(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        c @ ('`' | '*' | '=' | '~' | '_') => c,
        _ => '\0',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Types `keys` into an empty line (`<` is Backspace) and returns the
    /// line with the cursor shown as `|`.
    fn typed(start: &str, keys: &str) -> String {
        let mut e = Editor::from_text(start);
        e.col = start.chars().count();
        for c in keys.chars() {
            if c == '<' {
                e.backspace_pair();
            } else {
                e.type_char(c);
            }
        }
        let line = &e.lines[e.row];
        let at = line
            .char_indices()
            .nth(e.col)
            .map_or(line.len(), |(b, _)| b);
        format!("{}|{}", &line[..at], &line[at..])
    }

    #[test]
    fn brackets_pair_and_closers_step_over() {
        assert_eq!(typed("", "("), "(|)");
        assert_eq!(typed("", "call(x)"), "call(x)|");
        assert_eq!(typed("", "[[note"), "[[note|]]");
        assert_eq!(typed("", "[[note]]"), "[[note]]|");
        assert_eq!(typed("", "{a}"), "{a}|");
    }

    #[test]
    fn no_pair_right_before_a_word() {
        let mut e = Editor::from_text("word");
        e.type_char('(');
        assert_eq!(e.lines, ["(word"]);
    }

    #[test]
    fn backticks_pair_but_a_fence_can_be_typed() {
        assert_eq!(typed("", "`"), "`|`");
        assert_eq!(typed("", "`code`"), "`code`|");
        assert_eq!(typed("", "```"), "```|");
    }

    #[test]
    fn double_delimiters_pair_on_the_second_char() {
        assert_eq!(
            typed("", "*"),
            "*|",
            "a single * (bullet, italic) isn't paired"
        );
        assert_eq!(typed("", "**"), "**|**");
        assert_eq!(typed("", "**bold**"), "**bold**|");
        assert_eq!(typed("", "==hi=="), "==hi==|");
        assert_eq!(typed("", "~~no~~"), "~~no~~|");
        assert_eq!(typed("", "***x***"), "***x***|");
    }

    #[test]
    fn a_cursor_past_the_end_of_the_line_does_not_panic() {
        let mut e = Editor::from_text("ab");
        e.col = 5;
        e.type_char('*');
        e.backspace_pair();
        assert!(e.lines[0].starts_with("ab"));
    }

    #[test]
    fn backspace_deletes_an_empty_pair() {
        assert_eq!(typed("", "(<"), "|");
        assert_eq!(typed("", "**<<"), "|");
        assert_eq!(typed("", "(a<<"), "|");
        assert_eq!(typed("a)", "<"), "a|", "a lone closer is just deleted");
    }
}
