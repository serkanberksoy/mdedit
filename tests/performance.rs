//! Performance budgets: large documents must stay responsive. The limits
//! are generous (about 10× what a debug build needs), so they only catch
//! regressions like re-analyzing the document per screen row or a
//! quadratic scan, not normal noise.

use std::time::{Duration, Instant};

use mdedit::app::App;
use mdedit::blocks::analyze;
use mdedit::ui;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The feature showcase repeated to about 10,000 lines.
fn big_note() -> String {
    let showcase = include_str!("../example_mds/feature_showcase.md");
    let body = showcase.split_once("\n---\n").expect("frontmatter").1;
    body.repeat(40)
}

/// Average time of `f` over `n` runs (after one warm-up run).
fn time(n: u32, mut f: impl FnMut()) -> Duration {
    f();
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    start.elapsed() / n
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn a_deeply_nested_list_is_analyzed_in_linear_time() {
    let lines: Vec<String> = (0..3000)
        .map(|i| format!("{}- item", "  ".repeat(i.min(200))))
        .collect();
    let t = time(1, || {
        std::hint::black_box(analyze(&lines));
    });
    assert!(t < Duration::from_millis(200), "analyze took {t:?}");
}

#[test]
fn keys_on_a_10k_line_note_stay_fast() {
    let text = big_note();
    let mut app = App::new(&text, None);
    app.view.editor.row = app.view.editor.lines.len() / 2;
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut key = |app: &mut App, code| {
        press(app, code);
        term.draw(|f| ui::draw(f, app)).unwrap();
    };
    let typing = time(10, || key(&mut app, KeyCode::Char('a')));
    assert!(
        typing < Duration::from_millis(100),
        "typing took {typing:?}"
    );
    let down = time(10, || key(&mut app, KeyCode::Down));
    assert!(down < Duration::from_millis(100), "Down took {down:?}");
    let page = time(5, || key(&mut app, KeyCode::PageDown));
    assert!(page < Duration::from_millis(40), "Page Down took {page:?}");
}

#[test]
fn a_long_code_block_is_not_copied_every_frame() {
    let text = format!(
        "```rust\n{}```\ntext",
        "let x = foo(1, \"s\"); // c\n".repeat(30_000)
    );
    let mut app = App::new(&text, None);
    app.view.editor.row = app.view.editor.lines.len() - 1;
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let t = time(5, || {
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
    });
    assert!(t < Duration::from_millis(12), "a frame took {t:?}");
}
