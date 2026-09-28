//! The embedding API, used the way a host (an app with tabs and a file
//! list) would: only public items, several views sharing one `Shared`,
//! editors drawn into part of the screen, keys answered with outcomes, and
//! links found through the host's own resolver.

use std::fs;
use std::path::{Path, PathBuf};

use mdedit::resolver::Resolver;
use mdedit::shared::Shared;
use mdedit::ui::{EditorWidget, apply_terminal_workarounds};
use mdedit::view::{EditorView, Outcome, ViewMode};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Paragraph, Widget};

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("embedding")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn ctrl(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// The rows of `terminal`'s screen as text.
fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn an_editor_draws_into_part_of_the_screen() {
    let mut shared = Shared::new();
    shared.config.heading_size = mdedit::config::HeadingSize::On;
    let mut view = EditorView::new("# Title\nsecond line", None);
    view.editor.row = 1;
    let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
    terminal
        .draw(|frame| {
            // A host's file list on the left, the editor on the right.
            Paragraph::new("files\nnote.md").render(Rect::new(0, 0, 10, 6), frame.buffer_mut());
            let editor = Rect::new(10, 0, 30, 6);
            frame.render_stateful_widget(EditorWidget::new(&mut shared), editor, &mut view);
            apply_terminal_workarounds(frame.buffer_mut(), &shared.caps, &view.image_areas);
        })
        .unwrap();
    let screen = rows(&terminal);
    assert!(
        screen[0].starts_with("files     █ TITLE"),
        "{:?}",
        screen[0]
    );
    assert!(
        screen[1].starts_with("note.md   second line"),
        "{:?}",
        screen[1]
    );
    assert!(
        screen[5][10..].trim_start().starts_with("[no file]"),
        "status bar: {:?}",
        screen[5]
    );
    // The cursor is in the editor's area, at the start of line 2.
    assert_eq!(view.cursor, Some(Position::new(10, 1)));
    // Double-size headings span whole terminal rows: off in a narrower area.
    assert!(
        view.line_attrs
            .iter()
            .all(|a| *a == mdedit::wrap::LineAttr::Normal),
        "{:?}",
        view.line_attrs
    );
}

#[test]
fn without_a_status_bar_the_whole_area_is_text_and_prompts_use_its_last_row() {
    let mut shared = Shared::new();
    let mut view = EditorView::new("a\nb\nc", None);
    let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
    let draw =
        |terminal: &mut Terminal<TestBackend>, shared: &mut Shared, view: &mut EditorView| {
            terminal
                .draw(|frame| {
                    let widget = EditorWidget::new(shared).status_bar(false);
                    frame.render_stateful_widget(widget, frame.area(), view);
                })
                .unwrap();
        };
    draw(&mut terminal, &mut shared, &mut view);
    assert_eq!(rows(&terminal)[2].trim_end(), "c");
    assert!(view.status_line("").contains("Ln 1, Col 1"));
    view.handle_key(ctrl('f'), &mut shared);
    assert!(matches!(view.mode, ViewMode::Search(_)));
    draw(&mut terminal, &mut shared, &mut view);
    assert!(
        rows(&terminal)[2].starts_with(" Search: "),
        "{:?}",
        rows(&terminal)[2]
    );
}

#[test]
fn keys_only_the_host_can_act_on_come_back_as_outcomes() {
    let mut shared = Shared::new();
    let mut view = EditorView::new("text", None);
    assert_eq!(
        view.handle_key(ctrl('o'), &mut shared),
        Outcome::RequestOpen
    );
    assert_eq!(
        view.handle_key(ctrl('x'), &mut shared),
        Outcome::RequestClose
    );
    assert_eq!(
        view.handle_key(ctrl('s'), &mut shared),
        Outcome::RequestSaveAs,
        "untitled"
    );
    let save_as = key(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    assert_eq!(
        view.handle_key(save_as, &mut shared),
        Outcome::RequestSaveAs
    );
    // Keys mdedit doesn't use are the host's (e.g. switching tabs).
    assert_eq!(view.handle_key(ctrl('p'), &mut shared), Outcome::Ignored);
    assert_eq!(
        view.handle_key(key(KeyCode::F(5), KeyModifiers::NONE), &mut shared),
        Outcome::Ignored
    );
    assert_eq!(
        view.handle_key(key(KeyCode::Char('!'), KeyModifiers::NONE), &mut shared),
        Outcome::Consumed
    );
    assert!(view.is_dirty());
    assert_eq!(view.editor.lines, ["!text"]);
}

#[test]
fn a_view_saves_itself_and_under_a_new_name() {
    let d = tmp("save");
    let mut shared = Shared::new();
    let mut view = EditorView::new("text", None);
    assert!(view.save().is_err(), "untitled");
    view.save_as(d.join("a.md")).unwrap();
    assert_eq!(fs::read_to_string(d.join("a.md")).unwrap(), "text\n");
    assert_eq!(view.title(), "a.md");
    view.handle_key(key(KeyCode::Char('!'), KeyModifiers::NONE), &mut shared);
    assert_eq!(view.handle_key(ctrl('s'), &mut shared), Outcome::Consumed);
    assert_eq!(fs::read_to_string(d.join("a.md")).unwrap(), "!text\n");
    assert!(!view.is_dirty());
    let opened = EditorView::open(d.join("a.md")).unwrap();
    assert_eq!(opened.editor.lines, ["!text"]);
    assert!(EditorView::open(d.join("missing.md")).is_err());
}

/// A host's resolver: finds `[[Name]]` anywhere under a vault folder by
/// file name, like a vault index would.
struct Vault(PathBuf);

impl Resolver for Vault {
    fn resolve(&self, _from: Option<&Path>, target: &str) -> Result<PathBuf, String> {
        let name = if Path::new(target).extension().is_some() {
            target.to_string()
        } else {
            format!("{target}.md")
        };
        let mut dirs = vec![self.0.clone()];
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.file_name().is_some_and(|n| n == name.as_str()) {
                    return Ok(path);
                }
            }
        }
        Err(format!("{target} (not in the vault)"))
    }
}

#[test]
fn a_host_resolver_finds_links_and_embeds_its_own_way() {
    let d = tmp("vault");
    fs::create_dir_all(d.join("deep/down")).unwrap();
    fs::write(d.join("deep/down/Target.md"), "# Target\n## Part\ninside\n").unwrap();
    let mut shared = Shared::new();
    shared.resolver = Box::new(Vault(d.clone()));
    let note = d.join("note.md");
    let mut view = EditorView::new("[[Target#Part]]\n![[Target]]\n[[Nowhere]]", Some(note));

    // Following a link to another note is the host's job (e.g. a new tab).
    view.editor.col = 3;
    let follow = key(KeyCode::Enter, KeyModifiers::CONTROL);
    assert_eq!(
        view.handle_key(follow, &mut shared),
        Outcome::OpenLink {
            path: d.join("deep/down/Target.md"),
            heading: Some("Part".into())
        }
    );
    view.editor.row = 2;
    assert_eq!(view.handle_key(follow, &mut shared), Outcome::Consumed);
    assert!(view.status.contains("not in the vault"), "{}", view.status);

    // The embed on line 2 is found through the vault too.
    view.editor.row = 0;
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_stateful_widget(EditorWidget::new(&mut shared), frame.area(), &mut view);
        })
        .unwrap();
    let screen = rows(&terminal).join("\n");
    assert!(screen.contains("⧉ Target"), "{screen}");
    assert!(screen.contains("inside"), "{screen}");
}

#[test]
fn views_share_settings_but_not_documents() {
    let mut shared = Shared::new();
    shared.config.auto_pair = false;
    let mut one = EditorView::new("", None);
    let mut two = EditorView::new("", None);
    one.handle_key(key(KeyCode::Char('('), KeyModifiers::NONE), &mut shared);
    two.handle_key(key(KeyCode::Char('x'), KeyModifiers::NONE), &mut shared);
    assert_eq!(one.editor.lines, ["("], "the shared setting applies");
    assert_eq!(two.editor.lines, ["x"]);
    one.handle_key(ctrl('z'), &mut shared);
    assert_eq!(one.editor.lines, [""], "each view has its own undo");
    assert_eq!(two.editor.lines, ["x"]);
}
