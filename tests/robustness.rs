//! Robustness: no note may crash the renderer. Renders every note in
//! `example_mds/` and every feature fixture at several widths, with the
//! cursor on every line (raw and rendered views), and fuzzes the inline
//! parser with random markup and multi-byte text. Any panic fails the test.

use std::fs;
use std::path::{Path, PathBuf};

use mdedit::markdown::{inline_spans, render, render_source};
use mdedit::ui::render_document_in;
use mdedit::wrap::wrap;
use ratatui::style::Style;

fn notes() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    let mut dirs = vec![root.join("example_mds"), root.join("tests/fixtures")];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn every_note_renders_at_many_widths_and_with_the_cursor_on_every_line() {
    for path in notes() {
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<String> = text.lines().map(String::from).collect();
        let base = path.parent();
        for width in [0, 1, 7, 20, 125] {
            let _ = render_document_in(&lines, None, width, base);
        }
        for cursor in 0..lines.len() {
            let _ = render_document_in(&lines, Some(cursor), 80, base);
        }
    }
}

/// A tiny deterministic random generator (xorshift), so failures repeat.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

#[test]
fn random_markup_and_multibyte_text_never_panics() {
    let pieces = [
        "`",
        "``",
        "*",
        "**",
        "_",
        "__",
        "~~",
        "==",
        "\\",
        "[[",
        "]]",
        "|",
        "#",
        "![[",
        "](",
        ")",
        "[",
        "]",
        "http://x",
        " ",
        "  ",
        "\t",
        "a",
        "word",
        "é",
        "▸",
        "😀",
        "🇹🇷",
        "🧑\u{200d}🦳",
        "日本",
        "- ",
        "> ",
        "1. ",
        "- [x] ",
        "# ",
        ":",
        "^",
    ];
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..10_000 {
        let len = rng.next() % 12;
        let text: String = (0..len).map(|_| rng.pick(&pieces)).collect();
        let _ = inline_spans(&text, Style::default());
        for view in [render(&text), render_source(&text)] {
            for width in [0, 1, 3, 10] {
                let _ = wrap(&view.line, view.indent, width);
            }
        }
    }
}
