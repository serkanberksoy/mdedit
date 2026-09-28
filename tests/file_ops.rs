//! File operations (requirements/file_handling_requirements.md, F-01 … F-07),
//! driven through `App::handle_key` against real files in a temp directory.

use std::fs;
use std::path::{Path, PathBuf};

use mdedit::app::{Action, After, App, Mode, Purpose};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A fresh, empty directory for one test.
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("file_ops")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn press(app: &mut App, code: KeyCode) -> Action {
    app.handle_key(key(code, KeyModifiers::NONE))
}

fn ctrl(app: &mut App, c: char) -> Action {
    app.handle_key(key(KeyCode::Char(c), KeyModifiers::CONTROL))
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c));
    }
}

fn clear_input(app: &mut App) {
    for _ in 0..64 {
        press(app, KeyCode::Backspace);
    }
}

/// Opens Save As with the fallback key (works in every terminal).
fn save_as(app: &mut App) {
    app.handle_key(key(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    ));
}

fn browser(app: &App) -> &mdedit::app::Browser {
    match &app.mode {
        Mode::Browser(b) => b,
        other => panic!("expected the file browser, got {other:?}"),
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

// ---------------------------------------------------------------------------
// F-01 Save

#[test]
fn f01_ctrl_s_saves_existing_file_without_prompt() {
    let d = tmp("f01_save");
    let a = d.join("a.md");
    fs::write(&a, "old\n").unwrap();
    let mut app = App::new("old\n", Some(a.clone()));
    type_str(&mut app, "x");
    ctrl(&mut app, 's');
    assert_eq!(read(&a), "xold\n");
    assert!(matches!(app.mode, Mode::Edit));
    assert!(!app.view.editor.dirty);
    assert!(
        app.view.status.contains("Saved"),
        "status: {}",
        app.view.status
    );
}

#[test]
fn f01_untitled_ctrl_s_opens_save_as() {
    let d = tmp("f01_untitled");
    let mut app = App::new("new text\n", None);
    app.set_cwd(d.clone());
    ctrl(&mut app, 's');
    assert!(matches!(browser(&app).purpose, Purpose::SaveAs { .. }));
    press(&mut app, KeyCode::Enter);
    assert_eq!(read(&d.join("untitled.md")), "new text\n");
    assert_eq!(app.view.path, Some(d.join("untitled.md")));
}

#[test]
fn f01_write_error_keeps_document_unsaved() {
    let d = tmp("f01_error");
    let mut app = App::new("x\n", Some(d.join("missing").join("a.md")));
    type_str(&mut app, "y");
    ctrl(&mut app, 's');
    assert!(app.view.editor.dirty);
    assert!(matches!(app.mode, Mode::Edit));
    assert!(
        app.view.status.contains("Save failed"),
        "status: {}",
        app.view.status
    );
}

// ---------------------------------------------------------------------------
// F-02 Save As

#[test]
fn f02_save_as_writes_new_file_and_adopts_path() {
    let d = tmp("f02_new");
    let a = d.join("a.md");
    fs::write(&a, "old\n").unwrap();
    let mut app = App::new("hello\n", Some(a.clone()));

    save_as(&mut app);
    let b = browser(&app);
    assert!(matches!(b.purpose, Purpose::SaveAs { .. }));
    assert_eq!(b.dir, d);
    assert_eq!(b.input, "a.md");

    clear_input(&mut app);
    type_str(&mut app, "b");
    press(&mut app, KeyCode::Enter);

    assert_eq!(read(&d.join("b.md")), "hello\n", ".md is added");
    assert_eq!(read(&a), "old\n", "the old file is untouched");
    assert_eq!(app.view.path, Some(d.join("b.md")));
    assert!(matches!(app.mode, Mode::Edit));
    assert!(!app.view.editor.dirty);
    assert!(
        app.view.status.contains("Saved"),
        "status: {}",
        app.view.status
    );
}

#[test]
fn f02_ctrl_shift_s_opens_save_as() {
    let mut app = App::new("x\n", None);
    app.handle_key(key(
        KeyCode::Char('S'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ));
    assert!(matches!(browser(&app).purpose, Purpose::SaveAs { .. }));
}

#[test]
fn f02_untitled_starts_in_working_dir_as_untitled_md() {
    let d = tmp("f02_untitled");
    let mut app = App::new("x\n", None);
    app.set_cwd(d.clone());
    save_as(&mut app);
    let b = browser(&app);
    assert_eq!(
        (b.dir.as_path(), b.input.as_str()),
        (d.as_path(), "untitled.md")
    );
}

#[test]
fn f02_enter_folder_from_list_then_save() {
    let d = tmp("f02_folder");
    fs::create_dir(d.join("sub")).unwrap();
    let mut app = App::new("text\n", Some(d.join("a.md")));

    save_as(&mut app);
    let names: Vec<_> = browser(&app)
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(names, ["..", "sub"]);
    press(&mut app, KeyCode::Down); // ".."
    press(&mut app, KeyCode::Down); // "sub"
    press(&mut app, KeyCode::Enter);
    assert_eq!(browser(&app).dir, d.join("sub"));
    assert_eq!(browser(&app).selected, None, "back on the name field");

    press(&mut app, KeyCode::Enter);
    assert_eq!(read(&d.join("sub").join("a.md")), "text\n");
}

#[test]
fn f02_typed_relative_path() {
    let d = tmp("f02_path");
    fs::create_dir(d.join("sub")).unwrap();
    let mut app = App::new("text\n", Some(d.join("a.md")));
    save_as(&mut app);
    clear_input(&mut app);
    type_str(&mut app, "sub/c");
    press(&mut app, KeyCode::Enter);
    assert_eq!(read(&d.join("sub").join("c.md")), "text\n");
    assert_eq!(app.view.path, Some(d.join("sub").join("c.md")));
}

#[test]
fn f02_overwrite_needs_confirmation() {
    let d = tmp("f02_overwrite");
    fs::write(d.join("b.md"), "keep\n").unwrap();
    let mut app = App::new("new\n", Some(d.join("a.md")));
    save_as(&mut app);
    clear_input(&mut app);
    type_str(&mut app, "b.md");

    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::ConfirmOverwrite { .. }));
    press(&mut app, KeyCode::Char('n'));
    browser(&app);
    assert_eq!(read(&d.join("b.md")), "keep\n");

    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(read(&d.join("b.md")), "new\n");
    assert!(matches!(app.mode, Mode::Edit));
}

#[test]
fn f02_esc_cancels_without_writing() {
    let d = tmp("f02_cancel");
    let mut app = App::new("x\n", Some(d.join("a.md")));
    save_as(&mut app);
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Edit));
    assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
}

#[test]
fn f02_missing_folder_shows_error_and_stays_open() {
    let d = tmp("f02_missing");
    let mut app = App::new("x\n", Some(d.join("a.md")));
    save_as(&mut app);
    clear_input(&mut app);
    type_str(&mut app, "nope/x.md");
    press(&mut app, KeyCode::Enter);
    assert!(browser(&app).error.is_some());
    assert_eq!(
        app.view.path,
        Some(d.join("a.md")),
        "the document keeps its path"
    );
}

// ---------------------------------------------------------------------------
// F-03 Exit

#[test]
fn f03_clean_document_exits_immediately() {
    let d = tmp("f03_clean");
    let mut app = App::new("x\n", Some(d.join("a.md")));
    assert_eq!(ctrl(&mut app, 'x'), Action::Quit);
}

#[test]
fn f03_unsaved_asks_and_cancel_returns_to_editing() {
    let d = tmp("f03_cancel");
    let mut app = App::new("x\n", Some(d.join("a.md")));
    type_str(&mut app, "y");
    assert_eq!(ctrl(&mut app, 'x'), Action::Continue);
    assert!(matches!(app.mode, Mode::SavePrompt { .. }));
    assert_eq!(press(&mut app, KeyCode::Char('c')), Action::Continue);
    assert!(matches!(app.mode, Mode::Edit));

    ctrl(&mut app, 'x');
    assert_eq!(press(&mut app, KeyCode::Esc), Action::Continue);
    assert!(matches!(app.mode, Mode::Edit));
    assert!(app.view.editor.dirty);
}

#[test]
fn f03_no_exits_without_saving() {
    let d = tmp("f03_no");
    let a = d.join("a.md");
    fs::write(&a, "old\n").unwrap();
    let mut app = App::new("old\n", Some(a.clone()));
    type_str(&mut app, "y");
    ctrl(&mut app, 'x');
    assert_eq!(press(&mut app, KeyCode::Char('n')), Action::Quit);
    assert_eq!(read(&a), "old\n");
}

#[test]
fn f03_yes_saves_to_the_file_then_exits() {
    let d = tmp("f03_yes");
    let a = d.join("a.md");
    let mut app = App::new("old\n", Some(a.clone()));
    type_str(&mut app, "y");
    ctrl(&mut app, 'x');
    assert_eq!(press(&mut app, KeyCode::Char('y')), Action::Quit);
    assert_eq!(read(&a), "yold\n");
}

#[test]
fn f03_yes_on_untitled_goes_through_save_as() {
    let d = tmp("f03_untitled");
    let mut app = App::new("\n", None);
    app.set_cwd(d.clone());
    type_str(&mut app, "draft");
    ctrl(&mut app, 'x');
    press(&mut app, KeyCode::Char('y'));
    assert!(matches!(
        browser(&app).purpose,
        Purpose::SaveAs { then: After::Quit }
    ));

    // Cancelling Save As doesn't exit.
    assert_eq!(press(&mut app, KeyCode::Esc), Action::Continue);
    assert!(matches!(app.mode, Mode::Edit));

    ctrl(&mut app, 'x');
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(press(&mut app, KeyCode::Enter), Action::Quit);
    assert_eq!(read(&d.join("untitled.md")), "draft\n");
}

#[test]
fn f03_ctrl_q_no_longer_exits() {
    let mut app = App::new("x\n", None);
    assert_eq!(ctrl(&mut app, 'q'), Action::Continue);
}

// ---------------------------------------------------------------------------
// F-04 Open

/// A folder with the open document `a.md`, `b.md`, `notes.txt` and `sub/c.md`.
fn vault(name: &str) -> PathBuf {
    let d = tmp(name);
    fs::write(d.join("a.md"), "alpha\n").unwrap();
    fs::write(d.join("b.md"), "bravo\n").unwrap();
    fs::write(d.join("notes.txt"), "text\n").unwrap();
    fs::create_dir(d.join("sub")).unwrap();
    fs::write(d.join("sub").join("c.md"), "charlie\n").unwrap();
    d
}

fn open_a(d: &Path) -> App {
    App::new(&read(&d.join("a.md")), Some(d.join("a.md")))
}

fn names(app: &App) -> Vec<String> {
    browser(app)
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect()
}

/// Moves the highlight to the entry called `name`.
fn select(app: &mut App, name: &str) {
    let target = names(app).iter().position(|n| n == name).unwrap();
    while browser(app).selected != Some(target) {
        let down = browser(app).selected.is_none_or(|s| s < target);
        press(app, if down { KeyCode::Down } else { KeyCode::Up });
    }
}

#[test]
fn f04_picker_lists_folders_then_markdown_files() {
    let d = vault("f04_list");
    let mut app = open_a(&d);
    ctrl(&mut app, 'o');
    let b = browser(&app);
    assert_eq!(b.purpose, Purpose::Open);
    assert_eq!(b.dir, d);
    assert_eq!(names(&app), ["..", "sub", "a.md", "b.md"]);
    assert_eq!(browser(&app).selected, Some(1), "first entry after ..");

    ctrl(&mut app, 'h');
    assert!(
        names(&app).contains(&"notes.txt".to_string()),
        "Ctrl+H shows all files"
    );
}

#[test]
fn f04_enter_on_a_file_opens_it() {
    let d = vault("f04_open");
    let mut app = open_a(&d);
    app.view.editor.row = 0;
    app.view.editor.col = 3;
    ctrl(&mut app, 'o');
    select(&mut app, "b.md");
    press(&mut app, KeyCode::Enter);

    assert!(matches!(app.mode, Mode::Edit));
    assert_eq!(app.view.editor.to_text(), "bravo\n");
    assert_eq!(app.view.path, Some(d.join("b.md")));
    assert_eq!(
        (app.view.editor.row, app.view.editor.col, app.view.scroll),
        (0, 0, 0)
    );
    assert!(!app.view.editor.dirty);
    assert!(
        app.view.status.contains("Opened"),
        "status: {}",
        app.view.status
    );

    // Ctrl+S now saves to the opened file.
    type_str(&mut app, "!");
    ctrl(&mut app, 's');
    assert_eq!(read(&d.join("b.md")), "!bravo\n");
}

#[test]
fn f04_folders_are_entered_and_left() {
    let d = vault("f04_nav");
    let mut app = open_a(&d);
    ctrl(&mut app, 'o');
    select(&mut app, "sub");
    press(&mut app, KeyCode::Enter);
    assert_eq!(browser(&app).dir, d.join("sub"));
    assert_eq!(names(&app), ["..", "c.md"]);
    select(&mut app, "..");
    press(&mut app, KeyCode::Enter);
    assert_eq!(browser(&app).dir, d);
}

#[test]
fn f04_typing_filters_the_list() {
    let d = vault("f04_filter");
    let mut app = open_a(&d);
    ctrl(&mut app, 'o');
    type_str(&mut app, "b.m");
    assert_eq!(names(&app), ["..", "b.md"]);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.path, Some(d.join("b.md")));
}

#[test]
fn f04_typed_path_opens_that_file() {
    let d = vault("f04_path");
    let mut app = open_a(&d);
    ctrl(&mut app, 'o');
    type_str(&mut app, "sub/c.md");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.path, Some(d.join("sub").join("c.md")));
    assert_eq!(app.view.editor.to_text(), "charlie\n");
}

#[test]
fn f04_unsaved_changes_prompt_first() {
    let d = vault("f04_prompt");
    let mut app = open_a(&d);
    type_str(&mut app, "x");
    ctrl(&mut app, 'o');
    assert!(matches!(app.mode, Mode::SavePrompt { then: After::Open }));
    press(&mut app, KeyCode::Char('c'));
    assert!(matches!(app.mode, Mode::Edit));
    assert!(app.view.editor.dirty);
}

#[test]
fn f04_yes_saves_then_shows_the_picker() {
    let d = vault("f04_yes");
    let mut app = open_a(&d);
    type_str(&mut app, "x");
    ctrl(&mut app, 'o');
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(read(&d.join("a.md")), "xalpha\n");
    assert_eq!(browser(&app).purpose, Purpose::Open);
}

#[test]
fn f04_no_then_esc_keeps_the_changes() {
    let d = vault("f04_no_esc");
    let mut app = open_a(&d);
    type_str(&mut app, "x");
    ctrl(&mut app, 'o');
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(browser(&app).purpose, Purpose::Open);
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Edit));
    assert_eq!(app.view.editor.to_text(), "xalpha\n", "nothing is lost");
    assert!(app.view.editor.dirty);
}

#[test]
fn f04_no_then_open_discards_the_changes() {
    let d = vault("f04_no_open");
    let mut app = open_a(&d);
    type_str(&mut app, "x");
    ctrl(&mut app, 'o');
    press(&mut app, KeyCode::Char('n'));
    select(&mut app, "b.md");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.editor.to_text(), "bravo\n");
    assert_eq!(read(&d.join("a.md")), "alpha\n", "not saved");
}

#[test]
fn f04_unreadable_file_shows_error_and_keeps_document() {
    let d = vault("f04_bad");
    fs::write(d.join("bad.md"), [0xff, 0xfe, 0x00]).unwrap();
    let mut app = open_a(&d);
    ctrl(&mut app, 'o');
    select(&mut app, "bad.md");
    press(&mut app, KeyCode::Enter);
    assert!(browser(&app).error.is_some());
    assert_eq!(app.view.path, Some(d.join("a.md")));
    assert_eq!(app.view.editor.to_text(), "alpha\n");
}

#[test]
fn f04_opening_the_same_file_reloads_it() {
    let d = vault("f04_reload");
    let mut app = open_a(&d);
    fs::write(d.join("a.md"), "changed on disk\n").unwrap();
    ctrl(&mut app, 'o');
    select(&mut app, "a.md");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.view.editor.to_text(), "changed on disk\n");
}

#[test]
fn f04_untitled_starts_in_working_dir() {
    let d = vault("f04_untitled");
    let mut app = App::new("\n", None);
    app.set_cwd(d.clone());
    ctrl(&mut app, 'o');
    assert_eq!(browser(&app).dir, d);
}

// ---------------------------------------------------------------------------
// F-05 Follow link (Ctrl+Enter / Alt+Enter)

/// `a.md` (the open note) links to `b.md`, `sub/c.md` and headings.
fn linked(name: &str) -> (PathBuf, App) {
    let d = tmp(name);
    fs::create_dir(d.join("sub")).unwrap();
    fs::write(d.join("b.md"), "# First\ntext\n## Second\nmore\n").unwrap();
    fs::write(d.join("sub").join("c.md"), "charlie\n").unwrap();
    let a = "see [[b]] and [[sub/c|c]] and [x](sub/c.md)\n[[b#Second]] [[#Local]]\n## Local\nweb [w](https://example.com)\n";
    fs::write(d.join("a.md"), a).unwrap();
    let app = App::new(a, Some(d.join("a.md")));
    (d, app)
}

fn ctrl_enter(app: &mut App) -> Action {
    app.handle_key(key(KeyCode::Enter, KeyModifiers::CONTROL))
}

fn at(app: &mut App, row: usize, col: usize) {
    app.view.editor.row = row;
    app.view.editor.col = col;
}

#[test]
fn f05_a_link_to_this_note_jumps_within_it() {
    let d = tmp("f05_same_note");
    let text = "[[a#Second]] and [again](./a.md#second)\n## First\n## Second\n";
    fs::write(d.join("a.md"), text).unwrap();
    let mut app = App::new(text, Some(d.join("a.md")));
    press(&mut app, KeyCode::End);
    press(&mut app, KeyCode::Char('!')); // unsaved change
    for col in [3, 20] {
        at(&mut app, 0, col);
        ctrl_enter(&mut app);
        assert!(
            matches!(app.mode, Mode::Edit),
            "no save prompt: {:?}",
            app.mode
        );
        assert_eq!(
            app.view.editor.row, 2,
            "on the heading (link at column {col})"
        );
        assert!(
            app.view.editor.lines[0].ends_with('!'),
            "not reloaded from disk"
        );
        assert!(app.view.editor.dirty);
    }
}

#[test]
fn f05_a_reference_link_is_followed_through_its_definition() {
    let d = tmp("f05_reference");
    fs::write(d.join("b.md"), "# B\n## Part\nbody\n").unwrap();
    let text = "See [the notes][B notes].\n\n[b notes]: <b.md#Part>\n";
    fs::write(d.join("a.md"), text).unwrap();
    let mut app = App::new(text, Some(d.join("a.md")));
    at(&mut app, 0, 6);
    ctrl_enter(&mut app);
    assert_eq!(
        app.view.path.as_deref(),
        Some(d.join("b.md").as_path()),
        "{}",
        app.view.status
    );
    assert_eq!(app.view.editor.row, 1, "lands on the heading");
}

#[test]
fn f05_ctrl_enter_opens_the_linked_note() {
    let (d, mut app) = linked("f05_open");
    at(&mut app, 0, 6);
    ctrl_enter(&mut app);
    assert_eq!(app.view.path, Some(d.join("b.md")));
    assert_eq!(app.view.editor.lines[0], "# First");
    assert!(matches!(app.mode, Mode::Edit));
    assert!(
        app.view.status.contains("Opened"),
        "status: {}",
        app.view.status
    );
}

#[test]
fn f05_alias_folder_and_markdown_links() {
    let (d, mut app) = linked("f05_kinds");
    at(&mut app, 0, 20); // [[sub/c|c]]
    ctrl_enter(&mut app);
    assert_eq!(app.view.path, Some(d.join("sub").join("c.md")));

    let (d, mut app) = linked("f05_md");
    at(&mut app, 0, 40); // [x](sub/c.md)
    app.handle_key(key(KeyCode::Enter, KeyModifiers::ALT));
    assert_eq!(
        app.view.path,
        Some(d.join("sub").join("c.md")),
        "Alt+Enter too"
    );
}

#[test]
fn f05_heading_part_moves_the_cursor() {
    let (d, mut app) = linked("f05_heading");
    at(&mut app, 1, 3); // [[b#Second]]
    ctrl_enter(&mut app);
    assert_eq!(app.view.path, Some(d.join("b.md")));
    assert_eq!((app.view.editor.row, app.view.editor.col), (2, 0));
}

#[test]
fn f05_local_heading_link_stays_in_the_file() {
    let (d, mut app) = linked("f05_local");
    at(&mut app, 1, 16); // [[#Local]]
    ctrl_enter(&mut app);
    assert_eq!(app.view.path, Some(d.join("a.md")));
    assert_eq!(app.view.editor.row, 2);
}

#[test]
fn f05_unsaved_changes_prompt_then_save_discard_or_cancel() {
    // Cancel: nothing happens.
    let (d, mut app) = linked("f05_prompt");
    at(&mut app, 0, 0);
    type_str(&mut app, "!");
    at(&mut app, 0, 7);
    ctrl_enter(&mut app);
    assert!(matches!(
        app.mode,
        Mode::SavePrompt {
            then: After::FollowLink
        }
    ));
    press(&mut app, KeyCode::Char('c'));
    assert_eq!(app.view.path, Some(d.join("a.md")));
    assert!(app.view.editor.dirty);

    // No: changes are discarded, the link is followed.
    ctrl_enter(&mut app);
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.view.path, Some(d.join("b.md")));
    assert!(read(&d.join("a.md")).starts_with("see"), "not saved");

    // Yes: saved first, then followed.
    let (d, mut app) = linked("f05_yes");
    type_str(&mut app, "!");
    at(&mut app, 0, 7);
    ctrl_enter(&mut app);
    press(&mut app, KeyCode::Char('y'));
    assert!(read(&d.join("a.md")).starts_with("!see"));
    assert_eq!(app.view.path, Some(d.join("b.md")));
}

#[test]
fn f05_links_that_are_not_followed_say_why() {
    let (d, mut app) = linked("f05_not");
    at(&mut app, 3, 6); // [w](https://example.com)
    ctrl_enter(&mut app);
    assert!(
        app.view.status.contains("web"),
        "status: {}",
        app.view.status
    );
    assert_eq!(app.view.path, Some(d.join("a.md")));

    at(&mut app, 3, 1); // not on a link
    ctrl_enter(&mut app);
    assert!(
        app.view.status.contains("No link"),
        "status: {}",
        app.view.status
    );

    app.view.editor.lines[0] = "[[missing]]".into();
    at(&mut app, 0, 3);
    ctrl_enter(&mut app);
    assert!(
        app.view.status.contains("Not found"),
        "status: {}",
        app.view.status
    );
    assert!(matches!(app.mode, Mode::Edit));
}

#[test]
fn f05_untitled_resolves_from_the_working_directory() {
    let (d, _) = linked("f05_untitled");
    let mut app = App::new("[[b]]\n", None);
    app.set_cwd(d.clone());
    ctrl_enter(&mut app);
    assert_eq!(app.view.path, Some(d.join("b.md")));
}

// ---------------------------------------------------------------------------
// F-06 Land on a section (`file.md#Heading`)

#[test]
fn f06_go_to_heading_puts_it_at_the_top_of_the_screen() {
    let text: String =
        (0..40).map(|i| format!("line {i}\n")).collect::<String>() + "## My Title\nbody\n";
    let mut app = App::new(&text, None);
    assert!(app.go_to_heading("mytitle"));
    assert_eq!((app.view.editor.row, app.view.editor.col), (40, 0));
    assert_eq!(
        app.view.scroll, 40,
        "the heading is the first line on screen"
    );
}

#[test]
fn f06_unknown_heading_says_so_and_stays() {
    let mut app = App::new("# A\ntext\n", None);
    assert!(!app.go_to_heading("nope"));
    assert_eq!(app.view.editor.row, 0);
    assert!(
        app.view.status.contains("nope"),
        "status: {}",
        app.view.status
    );
}

#[test]
fn f06_following_a_heading_link_also_puts_it_at_the_top() {
    let (_, mut app) = linked("f06_link");
    at(&mut app, 1, 3); // [[b#Second]]
    ctrl_enter(&mut app);
    assert_eq!(app.view.scroll, 2);
}

// ---------------------------------------------------------------------------
// F-07 Safe save

/// Opens `path` the way `main` does, types `!` at the start and saves.
fn edit_and_save(path: &Path) {
    let text = read(path);
    let mut app = App::new(&text, Some(path.to_path_buf()));
    press(&mut app, KeyCode::Char('!'));
    ctrl(&mut app, 's');
    assert!(!app.view.editor.dirty, "saved: {}", app.view.status);
}

#[test]
fn f07_crlf_line_endings_are_kept() {
    let d = tmp("f07_crlf");
    let a = d.join("a.md");
    fs::write(&a, "one\r\ntwo\r\n").unwrap();
    edit_and_save(&a);
    assert_eq!(read(&a), "!one\r\ntwo\r\n");
}

#[test]
fn f07_a_missing_final_newline_stays_missing() {
    let d = tmp("f07_no_newline");
    let a = d.join("a.md");
    fs::write(&a, "one\ntwo").unwrap();
    edit_and_save(&a);
    assert_eq!(read(&a), "!one\ntwo");
}

#[test]
fn f07_save_leaves_no_temporary_file() {
    let d = tmp("f07_tmp");
    let a = d.join("a.md");
    fs::write(&a, "x\n").unwrap();
    edit_and_save(&a);
    let names: Vec<_> = fs::read_dir(&d)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(names, ["a.md"]);
}

#[cfg(unix)]
#[test]
fn f07_permissions_and_symlinks_are_kept() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("f07_unix");
    let real = d.join("real.md");
    fs::write(&real, "x\n").unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
    let link = d.join("link.md");
    std::os::unix::fs::symlink("real.md", &link).unwrap();
    edit_and_save(&link);
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(read(&real), "!x\n");
    let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn f07_a_failed_save_leaves_the_old_file() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("f07_fail");
    let a = d.join("a.md");
    fs::write(&a, "old\n").unwrap();
    // A read-only folder: the temporary file can't be created.
    fs::set_permissions(&d, fs::Permissions::from_mode(0o500)).unwrap();
    let mut app = App::new("new\n", Some(a.clone()));
    press(&mut app, KeyCode::Char('!'));
    ctrl(&mut app, 's');
    fs::set_permissions(&d, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(app.view.editor.dirty);
    assert_eq!(read(&a), "old\n");
}
