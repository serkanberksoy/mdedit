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
use ratatui::style::Modifier;
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
    let mut view = EditorView::new(
        "[[Target#Part]]\n![[Target]]\n[[Nowhere#Top]]\n[site](https://example.com/a)",
        Some(note),
    );

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
    // A link to a note that isn't there: the host may offer to make it.
    view.editor.row = 2;
    assert_eq!(
        view.handle_key(follow, &mut shared),
        Outcome::MissingLink {
            target: "Nowhere".into(),
            heading: Some("Top".into())
        }
    );
    assert!(view.status.contains("not in the vault"), "{}", view.status);
    // A web link: the host opens it (a browser).
    view.editor.row = 3;
    view.editor.col = 1;
    assert_eq!(
        view.handle_key(follow, &mut shared),
        Outcome::OpenUrl("https://example.com/a".into())
    );

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

/// A host's code block processor: `shout` blocks are shown in capitals.
struct Shout;

impl mdedit::processor::CodeBlockProcessor for Shout {
    fn handles(&self, lang: &str) -> bool {
        lang == "shout"
    }

    fn render(
        &self,
        _lang: &str,
        source: &[String],
        _from: Option<&Path>,
        width: usize,
    ) -> Vec<ratatui::text::Line<'static>> {
        let mut lines: Vec<_> = source.iter().map(|l| l.to_uppercase().into()).collect();
        lines.push(format!("width {width}").into());
        lines
    }
}

#[test]
fn a_host_renders_its_own_code_blocks_until_the_cursor_is_in_them() {
    let mut shared = Shared::new();
    shared.processor = Some(Box::new(Shout));
    let text = "top\n```shout\nhello\n```\n```rust\nlet x;\n```\nend";
    let mut view = EditorView::new(text, None);
    let mut terminal = Terminal::new(TestBackend::new(30, 12)).unwrap();
    let mut draw = |view: &mut EditorView, shared: &mut Shared| {
        terminal
            .draw(|frame| {
                let widget = EditorWidget::new(shared).status_bar(false);
                frame.render_stateful_widget(widget, frame.area(), view);
            })
            .unwrap();
        rows(&terminal)
            .iter()
            .map(|r| r.trim_end().to_string())
            .collect::<Vec<_>>()
    };
    let screen = draw(&mut view, &mut shared);
    assert_eq!(
        screen[..7],
        [
            "top",
            "╭─ shout",
            "│ HELLO",
            "│ width 28",
            "╰─",
            "╭─ rust",
            "│ let x;"
        ],
        "the block's result in place of its code; other languages as code"
    );
    // The cursor in the block: its source, to edit it.
    view.editor.row = 2;
    let screen = draw(&mut view, &mut shared);
    assert_eq!(screen[1..4], ["```shout", "hello", "```"]);
}

/// A processor whose result rows have actions: `pick` blocks list their
/// lines, each row acting as `pick:<line>`.
struct Pick;

impl mdedit::processor::CodeBlockProcessor for Pick {
    fn handles(&self, lang: &str) -> bool {
        lang == "pick"
    }

    fn render(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<ratatui::text::Line<'static>> {
        self.render_rows(lang, source, from, width)
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    fn render_rows(
        &self,
        _lang: &str,
        source: &[String],
        _from: Option<&Path>,
        _width: usize,
    ) -> Vec<(ratatui::text::Line<'static>, Option<String>)> {
        source
            .iter()
            .map(|l| (format!("• {l}").into(), Some(format!("pick:{l}"))))
            .collect()
    }
}

fn view_mode(view: &mut EditorView, shared: &mut Shared) {
    // Alt+V cycles: live preview → source → view.
    let cycle = key(KeyCode::Char('v'), KeyModifiers::ALT);
    view.handle_key(cycle, shared);
    view.handle_key(cycle, shared);
    assert!(view.reading, "view mode");
}

fn draw_view(
    terminal: &mut Terminal<TestBackend>,
    shared: &mut Shared,
    view: &mut EditorView,
) -> Vec<String> {
    terminal
        .draw(|frame| {
            let widget = EditorWidget::new(shared).status_bar(false);
            frame.render_stateful_widget(widget, frame.area(), view);
        })
        .unwrap();
    rows(terminal)
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect()
}

#[test]
fn view_mode_moves_over_rendered_rows_and_acts_on_them() {
    let mut shared = Shared::new();
    shared.processor = Some(Box::new(Pick));
    let text = "# Title\n```pick\napple\npear\n```\nend";
    let mut view = EditorView::new(text, None);
    view_mode(&mut view, &mut shared);
    let mut terminal = Terminal::new(TestBackend::new(30, 8)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[0], "█ TITLE", "the cursor line is rendered too");
    assert_eq!(
        screen[1..4],
        ["╭─ pick", "│ • apple", "│ • pear"],
        "{screen:?}"
    );
    assert_eq!(view.cursor, None, "no text cursor: a row cursor");
    // Down: the block's frame, then its rows.
    let down = key(KeyCode::Down, KeyModifiers::NONE);
    let enter = key(KeyCode::Enter, KeyModifiers::NONE);
    for _ in 0..3 {
        view.handle_key(down, &mut shared);
        draw_view(&mut terminal, &mut shared, &mut view);
    }
    assert_eq!(
        view.read_action(&shared),
        Some("pick:pear".into()),
        "the row's action, without acting"
    );
    assert_eq!(
        view.handle_key(enter, &mut shared),
        Outcome::Action("pick:pear".into())
    );
    // A click on a row acts too.
    draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(
        view.click(Position::new(4, 2), &mut shared),
        Outcome::Action("pick:apple".into())
    );
}

#[test]
fn view_mode_follows_links_with_tab_and_enter_and_edits_nothing() {
    let d = tmp("view-links");
    fs::write(d.join("other.md"), "# Other\n").unwrap();
    let mut shared = Shared::new();
    let note = d.join("note.md");
    let mut view = EditorView::new(
        "intro\nsee [[other]] and [[#Part]]\n## Part\ntext",
        Some(note),
    );
    view_mode(&mut view, &mut shared);
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    draw_view(&mut terminal, &mut shared, &mut view);
    let tab = key(KeyCode::Tab, KeyModifiers::NONE);
    let enter = key(KeyCode::Enter, KeyModifiers::NONE);
    view.handle_key(tab, &mut shared);
    assert_eq!(
        view.handle_key(enter, &mut shared),
        Outcome::OpenLink {
            path: d.join("other.md"),
            heading: None
        },
        "Tab went to the first link"
    );
    view.handle_key(tab, &mut shared);
    view.handle_key(enter, &mut shared);
    assert_eq!(
        view.editor.row, 2,
        "the second link: a heading in this note"
    );
    // Nothing edits the text.
    let before = view.editor.lines.clone();
    for code in [KeyCode::Char('x'), KeyCode::Backspace, KeyCode::Delete] {
        view.handle_key(key(code, KeyModifiers::NONE), &mut shared);
    }
    view.handle_paste("pasted", &mut shared);
    view.handle_key(ctrl('t'), &mut shared);
    assert_eq!(view.editor.lines, before);
    assert!(!view.is_dirty());
    assert!(view.status.contains("read-only"), "{}", view.status);
    // Esc goes back to editing.
    view.handle_key(key(KeyCode::Esc, KeyModifiers::NONE), &mut shared);
    assert!(!view.reading && !view.source_mode);
}

#[test]
fn a_click_on_a_link_in_the_live_preview_follows_it() {
    let d = tmp("live-click");
    fs::write(d.join("other.md"), "# Other\n").unwrap();
    let mut shared = Shared::new();
    let mut view = EditorView::new("first line\nsee [[other]] here", Some(d.join("note.md")));
    let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[1], "see other here", "rendered: not the cursor line");
    assert_eq!(
        view.click(Position::new(6, 1), &mut shared),
        Outcome::OpenLink {
            path: d.join("other.md"),
            heading: None
        }
    );
    assert_eq!(
        view.click(Position::new(1, 1), &mut shared),
        Outcome::Consumed,
        "not on the link: the cursor goes there"
    );
    assert_eq!((view.editor.row, view.editor.col), (1, 1));
    // On the line being edited (raw text), the cursor moves in it.
    draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(
        view.click(Position::new(6, 1), &mut shared),
        Outcome::Consumed
    );
    assert_eq!((view.editor.row, view.editor.col), (1, 6));
}

/// The char column of `needle` in a drawn row.
fn x_of(row: &str, needle: &str) -> u16 {
    row[..row.find(needle).expect("on the row")].chars().count() as u16
}

/// Where `needle` is drawn (the first row that has it).
fn at(screen: &[String], needle: &str) -> Position {
    let y = screen
        .iter()
        .position(|r| r.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} isn't drawn: {screen:#?}"));
    Position::new(x_of(&screen[y], needle), y as u16)
}

#[test]
fn a_click_in_the_text_places_the_cursor_where_the_source_is() {
    let mut shared = Shared::new();
    let text = "first line\n## A **bold** word\n- an item with `code` in it\nlast";
    let mut view = EditorView::new(text, Some(tmp("click-cursor").join("note.md")));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    // A rendered heading: markers hidden, the cursor lands in the source.
    assert_eq!(
        view.click(at(&screen, "word"), &mut shared),
        Outcome::Consumed
    );
    assert_eq!(
        (view.editor.row, view.editor.col),
        (1, "## A **bold** ".chars().count())
    );
    // A bullet (drawn as a glyph) and a code span.
    view.click(at(&screen, "in it"), &mut shared);
    assert_eq!(
        (view.editor.row, view.editor.col),
        (2, "- an item with `code` ".chars().count())
    );
    // The line being edited is raw: the click's column is the text's.
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert!(
        screen.iter().any(|r| r == "- an item with `code` in it"),
        "{screen:#?}"
    );
    view.click(at(&screen, "item"), &mut shared);
    assert_eq!((view.editor.row, view.editor.col), (2, 5));
    // Past the end of a line: its end.
    let last = at(&screen, "last");
    view.click(Position::new(30, last.y), &mut shared);
    assert_eq!((view.editor.row, view.editor.col), (3, 4));
    // A selection ends.
    view.editor.anchor = Some((0, 0));
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    let first = at(&screen, "first");
    view.click(Position::new(2, first.y), &mut shared);
    assert_eq!(
        (view.editor.row, view.editor.col, view.editor.anchor),
        (0, 2, None)
    );
}

#[test]
fn a_click_in_a_wrapped_line_and_in_source_mode() {
    let mut shared = Shared::new();
    let long = "word ".repeat(12);
    let text = format!("top\n{}end", long);
    let mut view = EditorView::new(&text, Some(tmp("click-wrap").join("note.md")));
    let mut terminal = Terminal::new(TestBackend::new(20, 8)).unwrap();
    view.editor.row = 1;
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    // The second screen row of the line being edited.
    assert!(screen[2].starts_with("word"), "{screen:#?}");
    view.click(Position::new(1, 2), &mut shared);
    // The first row's text and the space it broke at.
    let first_row = screen[1].trim_end().chars().count() + 1;
    assert_eq!((view.editor.row, view.editor.col), (1, first_row + 1));
    // Source mode: every line raw.
    view.source_mode = true;
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[0], "top");
    view.click(Position::new(2, 0), &mut shared);
    assert_eq!((view.editor.row, view.editor.col), (0, 2));
}

#[test]
fn a_links_click_area_is_the_link_not_the_same_word_before_it() {
    let d = tmp("same-word");
    fs::write(d.join("Charts.md"), "# Charts\n").unwrap();
    let mut shared = Shared::new();
    let text = "first line\ncharts and queries in [[Charts]] here\nCharts, again: [[Charts]]";
    let mut view = EditorView::new(text, Some(d.join("note.md")));
    let mut terminal = Terminal::new(TestBackend::new(60, 6)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[1], "charts and queries in Charts here");
    let link_x = "charts and queries in ".len() as u16 + 1;
    assert_eq!(
        view.click(Position::new(link_x, 1), &mut shared),
        Outcome::OpenLink {
            path: d.join("Charts.md"),
            heading: None
        },
        "the link itself"
    );
    assert_eq!(
        view.click(Position::new(1, 1), &mut shared),
        Outcome::Consumed,
        "the plain word with the same letters: the cursor goes there"
    );
    // The same word with the same capital before the link.
    let link_x = "Charts, again: ".len() as u16 + 1;
    assert!(matches!(
        view.click(Position::new(link_x, 2), &mut shared),
        Outcome::OpenLink { .. }
    ));
    view.editor.row = 0;
    draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(
        view.click(Position::new(1, 2), &mut shared),
        Outcome::Consumed
    );
}

#[test]
fn a_host_names_a_document_without_a_file() {
    let mut view = EditorView::new("text", None);
    assert_eq!(view.title(), "untitled");
    view.name = Some("Diff: Note.md".into());
    assert_eq!(view.title(), "Diff: Note.md");
    assert!(
        view.status_line("").contains("Diff: Note.md"),
        "{}",
        view.status_line("")
    );
}

#[test]
fn a_host_fills_a_margin_beside_the_lines() {
    let mut shared = Shared::new();
    let mut view = EditorView::new("one\ntwo\nthree", None);
    view.margin_width = 2;
    view.margin.insert(1, ratatui::text::Line::from("+"));
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[0], "  one");
    assert_eq!(screen[1], "+ two", "the host's mark, then the text");
    assert_eq!(view.text_area.x, 2, "clicks and the cursor follow the text");
    // Without a width there's no margin.
    view.margin_width = 0;
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[1], "two");
}

#[test]
fn links_to_missing_notes_are_dimmed() {
    let d = tmp("unresolved");
    fs::write(d.join("here.md"), "# Here\n").unwrap();
    let mut shared = Shared::new();
    let mut view = EditorView::new(
        "top\nsee [[here]] and [[gone|Gone]] and [[here#Part]]",
        Some(d.join("note.md")),
    );
    let mut terminal = Terminal::new(TestBackend::new(60, 4)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[1], "see here and Gone and here › Part");
    let style = |needle: &str| {
        let p = at(&screen, needle);
        terminal.backend().buffer()[(p.x, p.y)].style()
    };
    assert!(
        style("Gone").add_modifier.contains(Modifier::DIM),
        "missing: dimmed"
    );
    assert!(!style("here and").add_modifier.contains(Modifier::DIM));
    assert_eq!(
        style("Gone").fg,
        style("here and").fg,
        "still a link's color"
    );
}

#[test]
fn a_host_hides_lines_until_the_cursor_is_on_them() {
    let mut shared = Shared::new();
    mdedit::markdown::set_hidden_lines(Some(std::rc::Rc::new(|line: &str| {
        line.trim_start().starts_with("<!-- TBLFM:")
    })));
    let mut view = EditorView::new("top\n<!-- TBLFM: $2=1 -->\nend", None);
    let mut terminal = Terminal::new(TestBackend::new(30, 5)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[..2], ["top", "end"], "hidden: {screen:?}");
    // The cursor goes onto it: shown, to edit.
    view.handle_key(key(KeyCode::Down, KeyModifiers::NONE), &mut shared);
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(view.editor.row, 1);
    assert_eq!(
        screen[..3],
        ["top", "<!-- TBLFM: $2=1 -->", "end"],
        "{screen:?}"
    );
    mdedit::markdown::set_hidden_lines(None);
    view.handle_key(key(KeyCode::Down, KeyModifiers::NONE), &mut shared);
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    assert_eq!(screen[1], "<!-- TBLFM: $2=1 -->", "without the hook, shown");
}

#[test]
fn a_host_shows_its_own_table_cells_until_the_cursor_is_in_the_table() {
    let mut shared = Shared::new();
    mdedit::markdown::set_table_cells(Some(std::rc::Rc::new(|lines: &[String]| {
        Some(lines.iter().map(|l| l.replace("=1+1", "2")).collect())
    })));
    let mut view = EditorView::new("top\n| a | b |\n|---|---|\n| x | =1+1 |\nend", None);
    let mut terminal = Terminal::new(TestBackend::new(30, 9)).unwrap();
    let screen = draw_view(&mut terminal, &mut shared, &mut view);
    let all = screen.join("\n");
    assert!(all.contains("│ x │ 2 │") && !all.contains("=1+1"), "{all}");
    // In the table: as written.
    view.handle_key(key(KeyCode::Down, KeyModifiers::NONE), &mut shared);
    let all = draw_view(&mut terminal, &mut shared, &mut view).join("\n");
    assert!(all.contains("=1+1"), "{all}");
    mdedit::markdown::set_table_cells(None);
}
