//! Real-terminal regression test (R-18): runs the mdedit binary inside
//! Konsole, scrolls a real note, reads Konsole's screen back over D-Bus and
//! compares it with what ratatui drew for the same keys at the same size.
//! Catches terminal-specific wide-character bugs that the vt100 test can't.
//!
//! It opens a Konsole window, so it's ignored by default:
//!   cargo test --test konsole -- --ignored
//! Needs `konsole`, `qdbus` and `python3`; skips (passes) without them.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mdedit::app::App;
use mdedit::ui::draw;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthStr;

fn have(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The screen ratatui draws after `keys`, one string per row.
fn expected_screen(text: &str, path: &Path, keys: &str, rows: u16, cols: u16) -> Vec<String> {
    let mut app = App::new(text, Some(path.to_path_buf()));
    let mut term = Terminal::new(TestBackend::new(cols, rows)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    for k in keys.chars() {
        let (code, mods) = match k {
            'd' => (KeyCode::Down, KeyModifiers::NONE),
            'u' => (KeyCode::Up, KeyModifiers::NONE),
            'e' => (KeyCode::End, KeyModifiers::NONE),
            'h' => (KeyCode::Home, KeyModifiers::NONE),
            'E' => (KeyCode::Char('e'), KeyModifiers::CONTROL),
            other => panic!("unknown key {other}"),
        };
        app.handle_key(KeyEvent::new(code, mods));
        term.draw(|f| draw(f, &mut app)).unwrap();
    }
    let buf = term.backend().buffer();
    (0..rows)
        .map(|y| {
            let mut row = String::new();
            let mut x = 0;
            while x < cols {
                let sym = buf[(x, y)].symbol();
                row.push_str(sym);
                x += (sym.width() as u16).max(1);
            }
            // Konsole's text dump leaves out variation selectors (U+FE0F);
            // it still draws those emoji 2 columns wide, as ratatui does
            // (measured with a cursor-position query).
            row.replace('\u{fe0f}', "").trim_end().to_string()
        })
        .collect()
}

/// Runs `keys` in a real Konsole and returns its screen, or `None` if the
/// tools aren't available.
fn konsole_screen(file: &Path, keys: &str, work: &Path) -> Option<(Vec<String>, u16, u16)> {
    if !(have("konsole") && have("qdbus") && have("python3")) {
        eprintln!("skipping: needs konsole, qdbus and python3");
        return None;
    }
    let done = work.join("done");
    let _ = fs::remove_file(&done);
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/konsole_drive.py");
    let mut child = Command::new("konsole")
        .arg("--separate")
        .arg("-e")
        .arg("python3")
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_mdedit"))
        .arg(file)
        .arg(keys)
        .arg(&done)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start konsole");

    let start = Instant::now();
    while !done.exists() && start.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(200));
    }
    let size = fs::read_to_string(&done).expect("konsole_drive.py did not finish");
    let (rows, cols) = size.trim().split_once('x').expect("ROWSxCOLS");
    std::thread::sleep(Duration::from_millis(300));

    let out = Command::new("qdbus")
        .arg(format!("org.kde.konsole-{}", child.id()))
        .arg("/Sessions/1")
        .arg("getAllDisplayedTextList")
        .arg("false")
        .output()
        .expect("run qdbus");
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        out.status.success(),
        "qdbus failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let screen = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect();
    Some((screen, rows.parse().unwrap(), cols.parse().unwrap()))
}

#[test]
#[ignore = "opens a Konsole window; run with --ignored"]
fn scrolling_a_real_note_leaves_no_artifacts_in_konsole() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("konsole");
    fs::create_dir_all(&work).unwrap();
    let file = work.join("daily_note.md");
    let text = include_str!("../example_mds/daily_note.md");
    fs::write(&file, text).unwrap();
    // Down through the note and back up: every line switches between raw
    // and rendered, and long lines soft-wrap.
    let keys = format!("{}{}", "d".repeat(40), "u".repeat(40));

    let Some((actual, rows, cols)) = konsole_screen(&file, &keys, &work) else {
        return;
    };
    let expected = expected_screen(text, &file, &keys, rows, cols);
    for (y, (want, got)) in expected.iter().zip(&actual).enumerate() {
        assert_eq!(got, want, "Konsole row {y} differs from what was drawn");
    }
    assert_eq!(actual.len(), expected.len(), "row count");
}

#[test]
#[ignore = "opens a Konsole window; run with --ignored"]
fn emoji_picker_grid_lines_up_in_konsole() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("konsole-emoji");
    fs::create_dir_all(&work).unwrap();
    let file = work.join("note.md");
    fs::write(&file, "text\n").unwrap();
    // Open the picker and page down through every row of the grid, so each
    // emoji is drawn (and replaced) at least once.
    let keys = format!("E{}", "d".repeat(130));

    let Some((actual, rows, cols)) = konsole_screen(&file, &keys, &work) else {
        return;
    };
    let expected = expected_screen("text\n", &file, &keys, rows, cols);
    for (y, (want, got)) in expected.iter().zip(&actual).enumerate() {
        assert_eq!(got, want, "Konsole row {y} differs from what was drawn");
    }
}

#[test]
#[ignore = "opens a Konsole window; run with --ignored"]
fn tricky_emoji_in_a_note_line_up_in_konsole() {
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("konsole-tricky");
    fs::create_dir_all(&work).unwrap();
    let file = work.join("emoji.md");
    let text = "# Emoji\n\
        - flags 🇹🇷 🇪🇬 🇺🇸 end\n\
        - tag flag 🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f} end\n\
        - joined 🧑\u{200d}🦳 Robin end\n\
        - keycap #\u{fe0f}\u{20e3} and ⬆\u{fe0f} ❤\u{fe0f} end\n\
        - skin 👍🏽 end\n";
    fs::write(&file, text).unwrap();
    // Move over every line so each is drawn both raw and rendered.
    let keys = format!("{}{}", "d".repeat(6), "u".repeat(6));

    let Some((actual, rows, cols)) = konsole_screen(&file, &keys, &work) else {
        return;
    };
    let expected = expected_screen(text, &file, &keys, rows, cols);
    for (y, (want, got)) in expected.iter().zip(&actual).enumerate() {
        assert_eq!(got, want, "Konsole row {y} differs from what was drawn");
    }
}
