//! Live-preview rendering: every line is shown rendered, except the line under
//! the cursor, which is shown as raw Markdown source so it can be edited.
//!
//! [`EditorWidget`] draws an [`EditorView`] into any area (a host can put
//! several side by side); [`draw`] is the mdedit program's whole screen.
//! The document layout is in `doc`, prompts and dialogs in `popups`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use ratatui::Frame;
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::app::{After, App, Browser, Mode, Purpose};
use crate::blocks::{FoldKind, LineContext, Structure, TableRowKind, analyze_cached};
use crate::embed::section;
use crate::emoji::{self, Picker};
use crate::highlight::{Highlighted, highlight_cached};
use crate::images::draw_images;
use crate::links::{Link, embed_target};
use crate::markdown::{
    Options, Rendered, TAB, heading_style_with, render_code_line, render_code_pieces,
    render_continuation, render_fence_close, render_fence_open, render_frontmatter_line,
    render_link_definition, render_quote, render_source, render_source_with, render_table_row,
    render_with, table_border,
};
use crate::resolver::RelativeResolver;
use crate::shared::Shared;
use crate::terminal::{Capabilities, ascii_fallback, fit_color};
use crate::view::{EditorView, ViewMode};
use crate::wrap::{LineAttr, Wrapped, wrap};

mod doc;
mod popups;

use doc::{Doc, Links};
use popups::{draw_dialog, draw_view_popup};

/// The mdedit program's key hints in the status bar.
pub const KEY_HINTS: &str = "^O open ^S save ^⌥S save as ^X exit ^V source ^T task ^L close";

/// Draws an [`EditorView`] into an area: the document, and (unless turned
/// off) a status bar in the area's last row. The view's prompts (search,
/// replace) use that row, or the area's last row without a status bar; the
/// emoji picker is centered in the area. Afterwards the view knows where
/// the cursor goes ([`EditorView::cursor`]) and where images are.
///
/// Double-size headings only work in an area as wide as the screen (the
/// terminal sizes whole rows), so they're off in a narrower area.
pub struct EditorWidget<'a> {
    shared: &'a mut Shared,
    status_bar: bool,
    hints: &'a str,
}

impl<'a> EditorWidget<'a> {
    pub fn new(shared: &'a mut Shared) -> Self {
        EditorWidget {
            shared,
            status_bar: true,
            hints: "",
        }
    }

    /// Whether to draw the status bar (default: yes). A host with its own
    /// can show [`EditorView::status_line`] there instead.
    pub fn status_bar(mut self, show: bool) -> Self {
        self.status_bar = show;
        self
    }

    /// Key hints for the status bar (e.g. [`KEY_HINTS`]).
    pub fn hints(mut self, hints: &'a str) -> Self {
        self.hints = hints;
        self
    }
}

impl StatefulWidget for EditorWidget<'_> {
    type State = EditorView;

    fn render(self, area: Rect, buf: &mut Buffer, view: &mut EditorView) {
        let shared = self.shared;
        let (body, status) = if self.status_bar {
            let [body, status] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
            (body, status)
        } else {
            let last = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
            (area, last)
        };
        let (width, height) = (body.width as usize, body.height as usize);
        view.width = width;
        view.height = height;

        let full_width = area.x == buf.area.x && area.width == buf.area.width;
        let mut options = shared.render_options(view.source_mode);
        options.heading_sizes &= full_width;
        let ed = &view.editor;
        let links = Links {
            resolver: &*shared.resolver,
            from: view.path.as_deref(),
        };
        let doc = Doc::new(&ed.lines, Some(ed.row), options, &view.folds, Some(links))
            .with_selection(ed.selection())
            .with_image_cell(image_cell(shared));
        let rows_of = |i: usize| doc.rows(i, width);

        // Scroll (in source lines) so the cursor's screen row is visible.
        let (cursor_row, cursor_x) =
            rows_of(ed.row).position(display_offset(&ed.lines[ed.row], ed.col));
        view.scroll = view.scroll.min(ed.row);
        let mut heights: Vec<usize> = (view.scroll..ed.row)
            .map(|i| rows_of(i).rows.len())
            .collect();
        let mut above: usize = heights.iter().sum();
        while above + cursor_row >= height && view.scroll < ed.row {
            above -= heights.remove(0);
            view.scroll += 1;
        }

        let mut rows: Vec<Line> = Vec::with_capacity(height);
        let mut attrs: Vec<LineAttr> = Vec::with_capacity(height);
        // Images to draw over the text (E-04): screen row of the top, slot.
        let mut images = Vec::new();
        for i in view.scroll..ed.lines.len() {
            if rows.len() >= height {
                break;
            }
            let w = rows_of(i);
            if let Some(slot) = &w.image {
                images.push((rows.len() + slot.row, slot.clone()));
            }
            attrs.extend((0..w.rows.len()).map(|k| w.attrs.get(k).copied().unwrap_or_default()));
            rows.extend(w.rows);
        }
        rows.truncate(height);
        attrs.resize(height, LineAttr::Normal);
        Clear.render(body, buf);
        Paragraph::new(rows).render(body, buf);
        images.retain(|(y, _)| *y < height);
        view.image_areas = match &shared.picker {
            Some(picker) => draw_images(buf, body, &images, picker, &mut shared.image_cache),
            None => Vec::new(),
        };

        if self.status_bar {
            Paragraph::new(view.status_line(self.hints))
                .style(Style::default().bg(Color::DarkGray).fg(Color::White))
                .render(status, buf);
        }
        let x = cursor_x.min(width.saturating_sub(1));
        let y = (above + cursor_row).min(height.saturating_sub(1));
        let text_cursor = Position::new(body.x + x as u16, body.y + y as u16);
        view.cursor = draw_view_popup(buf, area, status, view).or(Some(text_cursor));
        view.line_attrs = attrs;
    }
}

/// The mdedit program's screen: the editor with its status bar, a dialog
/// if one is open, and the terminal workarounds.
pub fn draw(frame: &mut Frame, app: &mut App) {
    // What the terminal can do, after the user's overrides.
    let caps = app.shared.config.apply(app.shared.caps);
    let screen = frame.area();
    frame.render_stateful_widget(
        EditorWidget::new(&mut app.shared).hints(KEY_HINTS),
        screen,
        &mut app.view,
    );
    let cursor = draw_dialog(frame.buffer_mut(), screen, app).or(app.view.cursor);
    apply_terminal_workarounds(frame.buffer_mut(), &caps, &app.view.image_areas);
    if let Some(cursor) = cursor {
        frame.set_cursor_position(cursor);
    }
}

/// The terminal's cell size in pixels, for sizing images; `None` when
/// images are off.
fn image_cell(shared: &Shared) -> Option<(u16, u16)> {
    shared.picker.as_ref().map(|p| {
        let size = p.font_size();
        (size.width, size.height)
    })
}

/// [`render_document`] with embeds (`![[Note]]`) resolved relative to
/// `base` (the document's folder).
pub fn render_document_in(
    lines: &[String],
    cursor: Option<usize>,
    width: usize,
    base: Option<&std::path::Path>,
) -> Vec<Line<'static>> {
    render_document_folded(lines, cursor, width, base, &HashMap::new())
}

/// What `app` shows, as screen rows at `width` (no status bar): its
/// document with `cursor` shown raw, its folds, settings and mode, with
/// embeds resolved relative to `base`.
pub fn render_app(
    app: &App,
    cursor: Option<usize>,
    width: usize,
    base: Option<&Path>,
) -> Vec<Line<'static>> {
    let resolver = base.map(|b| RelativeResolver {
        cwd: b.to_path_buf(),
    });
    let links = resolver.as_ref().map(|resolver| Links {
        resolver,
        from: None,
    });
    let view = &app.view;
    let options = app.shared.render_options(view.source_mode);
    let doc = Doc::new(&view.editor.lines, cursor, options, &view.folds, links)
        .with_selection(view.editor.selection())
        .with_image_cell(image_cell(&app.shared));
    (0..view.editor.lines.len())
        .flat_map(|i| doc.rows(i, width).rows)
        .collect()
}

/// [`render_document_in`] with Ctrl+K fold choices ([`EditorView::folds`]).
pub fn render_document_folded(
    lines: &[String],
    cursor: Option<usize>,
    width: usize,
    base: Option<&Path>,
    folds: &HashMap<usize, bool>,
) -> Vec<Line<'static>> {
    let resolver = base.map(|b| RelativeResolver {
        cwd: b.to_path_buf(),
    });
    let links = resolver.as_ref().map(|resolver| Links {
        resolver,
        from: None,
    });
    let doc = Doc::new(lines, cursor, Options::default(), folds, links);
    (0..lines.len())
        .flat_map(|i| doc.rows(i, width).rows)
        .collect()
}

/// Renders the whole document as the editor shows it, soft-wrapped to
/// `width` columns (0 = no wrap), one entry per screen row. `cursor` is the
/// line shown as raw source; `None` renders every line formatted.
pub fn render_document(
    lines: &[String],
    cursor: Option<usize>,
    width: usize,
) -> Vec<Line<'static>> {
    render_document_in(lines, cursor, width, None)
}

/// The raw (cursor-line) view of `line`, wrapped to `width`. Vertical cursor
/// movement uses this to move by screen row.
pub fn raw_layout(line: &str, width: usize) -> Wrapped {
    let r = render_source(line);
    wrap(&r.line, r.indent, width)
}

/// Display column of char `col` in the raw view of `line` (tabs are [`TAB`]).
pub fn display_offset(line: &str, col: usize) -> usize {
    let before: String = line.chars().take(col).collect();
    before.replace('\t', TAB).width()
}

/// Char column of the grapheme at display column `offset` in the raw view of
/// `line` (the start of the grapheme it falls in, or the end of the line).
pub fn col_at_offset(line: &str, offset: usize) -> usize {
    let mut acc = 0;
    let mut col = 0;
    for g in line.graphemes(true) {
        let w = if g == "\t" { TAB.len() } else { g.width() };
        if acc + w > offset {
            return col;
        }
        acc += w;
        col += g.chars().count();
    }
    col
}

/// Fixes for terminals (seen in Konsole) that disagree with ratatui about
/// wide characters, and fallbacks for terminals with fewer colors or no
/// Unicode (`caps`). Display-only; the text is untouched. Run it once per
/// frame over the whole screen, after everything is drawn. Cells of
/// `images` are left as the image drew them (only their colors are fitted
/// to the terminal), so image data is never re-sent or mangled.
pub fn apply_terminal_workarounds(buffer: &mut Buffer, caps: &Capabilities, images: &[Rect]) {
    let area = buffer.area;
    for (i, cell) in buffer.content.iter_mut().enumerate() {
        let width = usize::from(area.width.max(1));
        let at = Position::new(area.x + (i % width) as u16, area.y + (i / width) as u16);
        if images.iter().any(|r| r.contains(at)) {
            cell.fg = fit_color(cell.fg, caps.colors);
            cell.bg = fit_color(cell.bg, caps.colors);
            continue;
        }
        if let Some(shown) = terminal_fallback(cell.symbol()) {
            cell.set_symbol(&shown);
        }
        // Terminals without Unicode get ASCII; fewer colors get the nearest.
        if !caps.unicode
            && let Some(shown) = ascii_fallback(cell.symbol())
        {
            cell.set_symbol(&shown);
        }
        cell.fg = fit_color(cell.fg, caps.colors);
        cell.bg = fit_color(cell.bg, caps.colors);
        // When a narrow char replaces a wide one, ratatui's diff skips the
        // wide char's trailing cell, trusting the terminal to have blanked it.
        // Konsole doesn't, so lines that shift sideways (raw <-> rendered,
        // scrolling) leave stale fragments. Repaint every cell in place.
        cell.set_diff_option(CellDiffOption::AlwaysUpdate);
    }
}

/// What to show instead of an emoji sequence that Konsole draws wider than
/// the 2 cells it's measured as (it swallows the text after it), if any.
/// Found with `tests/konsole.rs`:
/// - joined emoji (ZWJ, `🧑‍🦳`) → the first emoji (`🧑`);
/// - skin tones (`👍🏽`) → the emoji without the modifier (`👍`);
/// - flags (`🇹🇷`) → the country letters (`TR`);
/// - tag-sequence flags (`🏴` + tag chars) → the base `🏴`.
fn terminal_fallback(symbol: &str) -> Option<String> {
    let regional = |c: char| ('\u{1F1E6}'..='\u{1F1FF}').contains(&c);
    let tag = |c: char| ('\u{E0020}'..='\u{E007F}').contains(&c);
    let skin_tone = |c: char| ('\u{1F3FB}'..='\u{1F3FF}').contains(&c);

    if symbol.chars().count() == 2 && symbol.chars().all(regional) {
        let letters = symbol
            .chars()
            .map(|c| char::from(b'A' + (c as u32 - 0x1F1E6) as u8))
            .collect();
        return Some(letters);
    }
    let mut shown: String = symbol.split('\u{200d}').next().unwrap_or(symbol).into();
    shown.retain(|c| !tag(c) && !skin_tone(c));
    (shown != symbol && !shown.is_empty()).then_some(shown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn screen(app: &mut App, w: u16, h: u16) -> (Vec<String>, Position) {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        let rows = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        (rows, term.get_cursor_position().unwrap())
    }

    fn app(text: &str) -> App {
        App::new(text, None)
    }

    #[test]
    fn cursor_line_is_raw_others_rendered() {
        let mut a = app("# Title\n## Sub\n- item\n  - nested\nplain");
        a.view.editor.row = 2;
        let (rows, _) = screen(&mut a, 40, 7);
        assert_eq!(rows[0], "█ TITLE");
        assert_eq!(rows[1], "", "heading spacing (R-17)");
        assert_eq!(rows[2], "▌ Sub");
        assert_eq!(rows[3], "- item");
        assert_eq!(rows[4], "│ ◦ nested");
        assert_eq!(rows[5], "plain");
        assert!(rows[6].contains("Ln 3, Col 1"));
    }

    #[test]
    fn the_status_message_comes_before_the_key_hints() {
        let mut a = app("text");
        a.view.status = "Saved note.md".into();
        let (rows, _) = screen(&mut a, 60, 3);
        assert!(rows[2].contains("Saved note.md"), "{:?}", rows[2]);
    }

    #[test]
    fn moving_cursor_switches_which_line_is_raw() {
        let mut a = app("# Title\n- item");
        let (rows, _) = screen(&mut a, 30, 3);
        assert_eq!(rows[0..2], ["# Title", "• item"]);
        a.view.editor.down();
        let (rows, _) = screen(&mut a, 30, 3);
        assert_eq!(rows[0..2], ["█ TITLE", "- item"]);
    }

    #[test]
    fn tasks_render_as_checkboxes_off_cursor() {
        let mut a = app("- [ ] open\n- [x] closed\nx");
        a.view.editor.row = 2;
        let (rows, _) = screen(&mut a, 30, 4);
        assert_eq!(rows[0..2], ["☐ open", "☑ closed"]);
    }

    #[test]
    fn frontmatter_revealed_as_block() {
        let mut a = app("---\nk: v\n---\n# T");
        a.view.editor.row = 3;
        let (rows, _) = screen(&mut a, 44, 7);
        assert_eq!(rows[0], "┄".repeat(40));
        assert_eq!(rows[1], "k: v");
        a.view.editor.row = 1;
        let (rows, _) = screen(&mut a, 44, 7);
        assert_eq!(rows[0..5], ["---", "k: v", "---", "", "█ T"]);
    }

    /// An empty scratch folder for a UI test.
    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join("mdedit-ui-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn ctrl_alt(c: char) -> ratatui::crossterm::event::KeyEvent {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL | KeyModifiers::ALT)
    }

    #[test]
    fn save_as_popup_is_drawn_with_cursor_in_name_field() {
        let d = scratch("save_as_popup");
        std::fs::create_dir(d.join("sub")).unwrap();
        let mut a = App::new("text", Some(d.join("a.md")));
        a.handle_key(ctrl_alt('s'));
        let (rows, cursor) = screen(&mut a, 60, 14);
        let all = rows.join("\n");
        assert!(all.contains("Save As"), "{all}");
        assert!(all.contains(&d.display().to_string()), "{all}");
        assert!(all.contains("Name: a.md"), "{all}");
        assert!(all.contains("../") && all.contains("sub/"), "{all}");
        let name_row = rows.iter().position(|r| r.contains("Name: a.md")).unwrap();
        let row = &rows[name_row];
        let name_x = row[..row.find("a.md").unwrap()].chars().count() + 4;
        assert_eq!(cursor, Position::new(name_x as u16, name_row as u16));
    }

    #[test]
    fn exit_prompt_names_the_file_and_the_choices() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let d = scratch("exit_prompt");
        let mut a = App::new("text", Some(d.join("note.md")));
        a.handle_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
        a.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL));
        let (rows, _) = screen(&mut a, 70, 12);
        let all = rows.join("\n");
        assert!(all.contains("Save changes to note.md?"), "{all}");
        assert!(all.contains("[Y]es   [N]o   [C]ancel"), "{all}");
        assert!(all.contains("No: exit without saving"), "{all}");
    }

    #[test]
    fn open_picker_is_drawn_with_filter_field() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let d = scratch("open_popup");
        std::fs::write(d.join("b.md"), "b").unwrap();
        let mut a = App::new("text", Some(d.join("a.md")));
        a.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
        let (rows, _) = screen(&mut a, 60, 14);
        let all = rows.join("\n");
        assert!(all.contains(" Open "), "{all}");
        assert!(all.contains("Filter: "), "{all}");
        assert!(all.contains("b.md"), "{all}");
    }

    #[test]
    fn emoji_picker_shows_search_and_grid_with_cursor_in_search() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut a = app("text");
        a.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        for c in "gri".chars() {
            a.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let (rows, cursor) = screen(&mut a, 70, 16);
        let all = rows.join("\n");
        assert!(all.contains(" Emoji "), "{all}");
        assert!(all.contains("Search: gri"), "{all}");
        assert!(all.contains("😀"), "{all}");
        let row = rows.iter().position(|r| r.contains("Search: gri")).unwrap();
        let x = rows[row][..rows[row].find("gri").unwrap()].chars().count() + 3;
        assert_eq!(cursor, Position::new(x as u16, row as u16));
    }

    #[test]
    fn follow_link_prompt_names_the_target_and_the_consequence() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let d = scratch("follow_prompt");
        std::fs::write(d.join("b.md"), "b").unwrap();
        let mut a = App::new("[[b]]", Some(d.join("a.md")));
        a.handle_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
        a.view.editor.col = 3;
        a.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        let (rows, _) = screen(&mut a, 80, 12);
        let all = rows.join("\n");
        assert!(
            all.contains("Following link to b.md. Save changes to a.md?"),
            "{all}"
        );
        assert!(
            all.contains("No: discard the changes and follow the link"),
            "{all}"
        );
    }

    #[test]
    fn an_image_is_drawn_in_its_frame_with_half_blocks() {
        let d = scratch("image_draw");
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/04-embeds/diagram.png"
        );
        std::fs::copy(fixture, d.join("diagram.png")).unwrap();
        let mut a = App::new("text\n![[diagram.png]]\nafter", Some(d.join("a.md")));
        let mut term = Terminal::new(TestBackend::new(40, 8)).unwrap();
        term.draw(|f| draw(f, &mut a)).unwrap();
        let buf = term.backend().buffer();
        let row = |y: u16| -> String { (0..40).map(|x| buf[(x, y)].symbol()).collect() };
        assert!(row(1).starts_with("╭─ 🖼 diagram.png"), "{}", row(1));
        assert!(row(4).starts_with("╰─"), "{}", row(4));
        assert!(row(5).starts_with("after"), "{}", row(5));
        // 100×40 px in 10×20 px cells: 10 columns × 2 rows, from column 2.
        let red = Color::Rgb(255, 0, 0);
        let blue = Color::Rgb(0, 0, 255);
        let color = |x: u16, y: u16| buf[(x, y)].fg;
        assert_eq!(color(2, 2), red, "left half: {:?}", buf[(2, 2)]);
        assert_eq!(color(11, 3), blue, "right half: {:?}", buf[(11, 3)]);
        assert_eq!(buf[(12, 2)].symbol(), " ", "nothing past the image");
    }

    #[test]
    fn selected_lines_are_raw_with_the_selection_reversed() {
        let mut a = app("# Title\n**bold** word\nlast");
        a.view.editor.anchor = Some((0, 2));
        (a.view.editor.row, a.view.editor.col) = (1, 2);
        let mut term = Terminal::new(TestBackend::new(30, 5)).unwrap();
        term.draw(|f| draw(f, &mut a)).unwrap();
        let buf = term.backend().buffer();
        let row = |y: u16| -> String { (0..30).map(|x| buf[(x, y)].symbol()).collect() };
        let reversed = |x: u16, y: u16| buf[(x, y)].modifier.contains(Modifier::REVERSED);
        assert!(row(0).starts_with("# Title"), "raw: {}", row(0));
        assert!(row(1).starts_with("**bold** word"), "raw: {}", row(1));
        assert!(row(2).starts_with("last"));
        assert!(!reversed(1, 0) && (2..7).all(|x| reversed(x, 0)));
        assert!((0..2).all(|x| reversed(x, 1)) && !reversed(2, 1));
        assert!(!reversed(0, 2));
    }

    #[test]
    fn replace_prompt_shows_both_fields_with_cursor_in_the_active_one() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut a = app("alpha beta\nbeta");
        let press = |a: &mut App, code| a.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        a.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        for c in "beta".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        let (rows, cursor) = screen(&mut a, 100, 4);
        assert!(rows[3].starts_with(" Find: beta │ Replace:"), "{}", rows[3]);
        assert!(rows[3].contains("1/2"), "{}", rows[3]);
        assert_eq!(cursor, Position::new(11, 3));
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Char('x'));
        let (rows, cursor) = screen(&mut a, 100, 4);
        assert!(
            rows[3].starts_with(" Find: beta │ Replace: x"),
            "{}",
            rows[3]
        );
        assert_eq!(cursor, Position::new(24, 3));
    }

    #[test]
    fn search_prompt_replaces_the_status_bar() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut a = app("alpha beta\nbeta");
        a.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        for c in "beta".chars() {
            a.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let (rows, cursor) = screen(&mut a, 80, 4);
        assert!(rows[3].starts_with(" Search: beta   1/2"), "{}", rows[3]);
        assert_eq!(
            cursor,
            Position::new(13, 3),
            "cursor at the end of the query"
        );
    }

    fn embed_scratch(name: &str) -> std::path::PathBuf {
        let d = scratch(name);
        std::fs::write(d.join("inner.md"), "# Inner\ntext\n![[deeper]]\n").unwrap();
        d
    }

    fn rendered_in(text: &str, cursor: Option<usize>, base: &std::path::Path) -> Vec<String> {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        render_document_in(&lines, cursor, 40, Some(base))
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn embed_shows_the_note_and_the_link_while_editing() {
        let d = embed_scratch("embed_basic");
        assert_eq!(
            rendered_in("![[inner]]\nafter", None, &d),
            [
                "╭─ ⧉ inner",
                "│ █ INNER",
                "│ text",
                "│ ⧉ deeper",
                "╰─",
                "after"
            ],
            "embeds inside an embed stay links"
        );
        assert_eq!(
            rendered_in("![[inner]]\nafter", Some(0), &d),
            ["![[inner]]", "after"]
        );
    }

    #[test]
    fn missing_embed_says_not_found() {
        let d = embed_scratch("embed_missing");
        assert_eq!(
            rendered_in("![[nowhere]]", None, &d),
            ["╭─ ⚠ nowhere (not found)", "╰─"]
        );
    }

    #[test]
    fn embeds_follow_file_changes() {
        let d = embed_scratch("embed_changes");
        assert_eq!(rendered_in("![[inner]]", None, &d)[2], "│ text");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(d.join("inner.md"), "# Inner\nchanged\n").unwrap();
        assert_eq!(rendered_in("![[inner]]", None, &d)[2], "│ changed");
    }

    fn big_headings(text: &str, cursor: usize, w: u16, h: u16) -> (Vec<String>, Vec<LineAttr>) {
        let mut a = app(text);
        a.shared.caps.line_sizes = true;
        a.shared.config.heading_size = crate::config::HeadingSize::Auto;
        a.view.editor.row = cursor;
        let (rows, _) = screen(&mut a, w, h);
        (rows, a.view.line_attrs.clone())
    }

    #[test]
    fn level_1_headings_are_double_size_and_level_2_double_width() {
        use LineAttr::*;
        let (rows, attrs) = big_headings("# Big\ntext\n## Mid", 1, 40, 6);
        assert_eq!(rows[0..5], ["█ BIG", "█ BIG", "text", "", "▌ Mid"]);
        assert_eq!(
            attrs[0..5],
            [DoubleTop, DoubleBottom, Normal, Normal, DoubleWide]
        );
    }

    #[test]
    fn big_headings_wrap_at_half_the_width() {
        use LineAttr::*;
        let (rows, attrs) = big_headings("# aaa bbb ccc\nx", 1, 20, 7);
        assert_eq!(rows[0..4], ["█ AAA BBB", "█ AAA BBB", "  CCC", "  CCC"]);
        assert_eq!(
            attrs[0..4],
            [DoubleTop, DoubleBottom, DoubleTop, DoubleBottom]
        );
    }

    #[test]
    fn a_heading_being_edited_is_normal_size() {
        let (rows, attrs) = big_headings("# Big\ntext", 0, 40, 4);
        assert_eq!(rows[0], "# Big");
        assert!(attrs.iter().all(|a| *a == LineAttr::Normal));
    }

    #[test]
    fn heading_size_setting_overrides_detection() {
        use crate::config::HeadingSize;
        let mut a = app("# Big\nx");
        a.view.editor.row = 1;
        a.shared.config.heading_size = HeadingSize::On;
        screen(&mut a, 40, 4);
        assert_eq!(
            a.view.line_attrs[0],
            LineAttr::DoubleTop,
            "on without detection"
        );
        a.shared.caps.line_sizes = true;
        a.shared.config.heading_size = HeadingSize::Off;
        screen(&mut a, 40, 4);
        assert_eq!(
            a.view.line_attrs[0],
            LineAttr::Normal,
            "off despite detection"
        );
    }

    #[test]
    fn basic_terminals_get_no_rgb_colors_and_ascii_symbols() {
        use crate::terminal::ColorDepth;
        let mut a = app("# Title\n- [ ] task #tag\n> [!tip] T\n```rust\nlet x = 1;\n```\nend");
        a.view.editor.row = 6;
        a.shared.caps.colors = ColorDepth::Basic16;
        a.shared.caps.unicode = false;
        let mut term = Terminal::new(TestBackend::new(40, 10)).unwrap();
        let frame = term.draw(|f| draw(f, &mut a)).unwrap();
        for cell in frame.buffer.content.iter() {
            assert!(cell.symbol().is_ascii(), "non-ASCII {:?}", cell.symbol());
            for c in [cell.fg, cell.bg] {
                assert!(!matches!(c, Color::Rgb(..) | Color::Indexed(_)), "{c:?}");
            }
        }
    }

    #[test]
    fn source_mode_shows_every_line_raw() {
        let d = embed_scratch("source_mode");
        let mut a = App::new(
            "# T\ntext\n## S\n- [x] done\n\n| a |\n|---|\n| 1 |\n![[inner]]\n> [!tip]- F\n> hidden",
            Some(d.join("note.md")),
        );
        a.view.source_mode = true;
        a.view.editor.row = 1;
        let rows: Vec<String> = render_app(&a, Some(1), 40, Some(&d))
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(
            rows,
            [
                "# T",
                "text",
                "## S",
                "- [x] done",
                "",
                "| a |",
                "|---|",
                "| 1 |",
                "![[inner]]",
                "> [!tip]- F",
                "> hidden"
            ]
        );
    }

    #[test]
    fn status_bar_shows_source_mode() {
        let mut a = app("x");
        a.view.source_mode = true;
        let (rows, _) = screen(&mut a, 120, 3);
        assert!(rows[2].contains("SOURCE"), "{}", rows[2]);
        assert!(rows[2].contains("^V"), "{}", rows[2]);
    }

    #[test]
    fn status_bar_lists_file_keys() {
        let mut a = app("x");
        let (rows, _) = screen(&mut a, 100, 3);
        assert!(rows[2].contains("^X exit"), "{}", rows[2]);
        assert!(rows[2].contains("^O open"), "{}", rows[2]);
        assert!(!rows[2].contains("^Q"), "{}", rows[2]);
    }

    fn rendered(text: &str, cursor: Option<usize>) -> Vec<String> {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        render_document(&lines, cursor, 30)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn heading_after_text_gets_a_spacer_row() {
        assert_eq!(
            rendered("text\n# H\nmore", None),
            ["text", "", "█ H", "more"]
        );
    }

    #[test]
    fn no_spacer_after_a_blank_line_or_at_the_top() {
        assert_eq!(rendered("text\n\n# H", None), ["text", "", "█ H"]);
        assert_eq!(rendered("# H\nx", None), ["█ H", "x"]);
    }

    #[test]
    fn spacer_stays_while_editing_the_heading() {
        assert_eq!(rendered("text\n## H", Some(1)), ["text", "", "## H"]);
        let mut a = app("text\n## H");
        a.view.editor.row = 1;
        a.view.editor.col = 3;
        let (rows, cursor) = screen(&mut a, 20, 5);
        assert_eq!(rows[0..3], ["text", "", "## H"]);
        assert_eq!(cursor, Position::new(3, 2));
    }

    #[test]
    fn setext_heading_gets_the_spacer_not_its_underline() {
        assert_eq!(
            rendered("text\nTitle\n===", None),
            ["text", "", "█ TITLE", "═══════"]
        );
    }

    #[test]
    fn nested_continuation_keeps_guides_and_wraps_under_the_text() {
        let lines: Vec<String> = ["- a", "  - b", "    one two three four"]
            .map(String::from)
            .to_vec();
        let rows: Vec<String> = render_document(&lines, None, 14)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(rows, ["• a", "│ ◦ b", "│   one two", "    three four"]);
    }

    #[test]
    fn done_style_setting_is_used_on_screen() {
        use crate::markdown::DoneStyle;
        use ratatui::style::Modifier;
        let draw_done = |style: DoneStyle| {
            let mut a = app("x\n- [x] done");
            a.shared.config.done_style = style;
            let mut term = Terminal::new(TestBackend::new(20, 3)).unwrap();
            let frame = term.draw(|f| draw(f, &mut a)).unwrap();
            frame.buffer[(2, 1)].modifier // the "d" of "done"
        };
        assert!(draw_done(DoneStyle::Strike).contains(Modifier::CROSSED_OUT));
        assert!(!draw_done(DoneStyle::Grey).contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn collapsed_callout_hides_its_body() {
        assert_eq!(
            rendered("> [!tip]- T\n> body\nafter", None),
            ["┃ ✦ T ▸", "after"]
        );
        assert_eq!(rendered("> [!tip]+ T\n> body", None), ["┃ ✦ T ▾", "┃ body"]);
    }

    #[test]
    fn folded_sections_and_items_say_how_much_is_hidden() {
        let mut a = app("# A\ntext\nmore\n- item\n  - sub\nend");
        a.view.folds.insert(3, true);
        a.view.editor.row = 5;
        let (rows, _) = screen(&mut a, 30, 8);
        assert_eq!(
            rows[0..5],
            ["█ A", "text", "more", "• item ▸ 1 line", "end"]
        );
        a.view.folds.insert(0, true);
        a.view.editor.row = 0;
        let (rows, _) = screen(&mut a, 30, 8);
        assert_eq!(
            rows[0], "# A ▸ 5 lines",
            "shown while editing the heading too"
        );
    }

    #[test]
    fn fold_overrides_change_what_is_shown() {
        let mut a = app("> [!tip]- T\n> body\nafter");
        a.view.folds.insert(0, false);
        let (rows, _) = screen(&mut a, 20, 5);
        assert_eq!(rows[0..3], ["> [!tip]- T", "┃ body", "after"]);
    }

    #[test]
    fn tables_are_drawn_with_borders_unless_being_edited() {
        let text = "| a | bb |\n|---|---|\n| 1 | 2 |\nx";
        assert_eq!(
            rendered(text, None),
            [
                "┌───┬────┐",
                "│ a │ bb │",
                "├───┼────┤",
                "│ 1 │ 2  │",
                "└───┴────┘",
                "x"
            ]
        );
        assert_eq!(
            rendered(text, Some(2)),
            ["| a | bb |", "|---|---|", "| 1 | 2 |", "x"]
        );
    }

    fn body_colors(text: &str) -> std::collections::HashSet<Option<ratatui::style::Color>> {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        let rows = render_document(&lines, None, 60);
        rows[1]
            .spans
            .iter()
            .skip(1) // the "│ " frame
            .filter(|s| !s.content.trim().is_empty())
            .map(|s| s.style.fg)
            .collect()
    }

    #[test]
    fn code_with_a_known_language_is_highlighted() {
        assert!(body_colors("```rust\nlet s = \"hi\";\n```").len() >= 2);
    }

    #[test]
    fn code_without_a_known_language_uses_the_code_color() {
        use ratatui::style::Color;
        let one = std::collections::HashSet::from([Some(Color::LightYellow)]);
        assert_eq!(body_colors("```nosuchlang\nlet s = 1;\n```"), one);
        assert_eq!(body_colors("```\nlet s = 1;\n```"), one);
    }

    #[test]
    fn long_line_wraps_with_hanging_indent() {
        let mut a = app("- aaa bbb ccc ddd\nx");
        a.view.editor.row = 1;
        let (rows, _) = screen(&mut a, 10, 4);
        assert_eq!(rows[0..3], ["• aaa bbb", "  ccc ddd", "x"]);
    }

    #[test]
    fn cursor_on_wrapped_raw_line() {
        let mut a = app("aaaa bbbb cccc");
        a.view.editor.col = 12;
        let (rows, cursor) = screen(&mut a, 10, 3);
        assert_eq!(rows[0..2], ["aaaa bbbb", "cccc"]);
        assert_eq!(cursor, Position::new(2, 1));
    }

    #[test]
    fn raw_cursor_line_wraps_under_its_marker() {
        let mut a = app("- [ ] aaa bbb ccc");
        a.view.editor.col = 17; // end of line
        let (rows, cursor) = screen(&mut a, 13, 3);
        assert_eq!(rows[0..2], ["- [ ] aaa bbb", "      ccc"]);
        assert_eq!(cursor, Position::new(9, 1));
    }

    #[test]
    fn scroll_counts_wrapped_rows() {
        let mut a = app("l0 aaaa bbbb\nl1 aaaa bbbb\nl2 aaaa bbbb\nl3 aaaa bbbb");
        a.view.editor.row = 3;
        let (rows, cursor) = screen(&mut a, 10, 5);
        assert_eq!(rows[0..4], ["l2 aaaa", "bbbb", "l3 aaaa", "bbbb"]);
        assert_eq!(cursor, Position::new(0, 2));
    }

    #[test]
    fn render_document_wraps_to_width() {
        let lines = vec!["aaaa bbbb cccc".to_string()];
        let plain: Vec<String> = render_document(&lines, None, 10)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(plain, ["aaaa bbbb", "cccc"]);
    }

    #[test]
    fn code_block_revealed_as_a_whole() {
        let mut a = app("```rust\nlet x;\n```\nafter");
        a.view.editor.row = 3;
        let (rows, _) = screen(&mut a, 30, 5);
        assert_eq!(rows[0..3], ["╭─ rust", "│ let x;", "╰─"]);
        a.view.editor.row = 1;
        let (rows, _) = screen(&mut a, 30, 5);
        assert_eq!(rows[0..4], ["```rust", "let x;", "```", "after"]);
    }

    #[test]
    fn cursor_accounts_for_tabs() {
        let mut a = app("\t- [.] log");
        a.view.editor.col = 3;
        let (rows, cursor) = screen(&mut a, 30, 3);
        assert_eq!(rows[0], "    - [.] log");
        assert_eq!(cursor, Position::new(6, 0));
    }

    #[test]
    fn zwj_emoji_shown_as_first_emoji() {
        let mut a = app("x\nfor [[🧑\u{200d}🦳 Robin]]");
        let (rows, _) = screen(&mut a, 30, 3);
        assert_eq!(rows[1], "for 🧑  Robin"); // wide cell + its trailing cell
    }

    #[test]
    fn flags_are_shown_as_their_country_letters() {
        // Konsole draws regional-indicator pairs wider than 2 cells, so the
        // screen shows the letters instead (the file keeps the flag).
        let mut a = app("x\n🇹🇷 🇪🇬 and 🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f}!");
        let (rows, _) = screen(&mut a, 30, 3);
        assert_eq!(rows[1], "TR  EG  and 🏴 !"); // wide cell + its trailing cell
        assert!(a.view.editor.lines[1].starts_with("🇹🇷"));
    }

    #[test]
    fn skin_tones_are_shown_without_the_modifier() {
        // Konsole draws emoji + skin-tone modifier wider than 2 cells.
        let mut a = app("x\n👍🏽 ok");
        let (rows, _) = screen(&mut a, 30, 3);
        assert_eq!(rows[1], "👍  ok");
        assert_eq!(a.view.editor.lines[1], "👍🏽 ok");
    }

    #[test]
    fn every_cell_is_repainted() {
        let mut term = Terminal::new(TestBackend::new(20, 3)).unwrap();
        let mut a = app("😀 hi");
        let frame = term.draw(|f| draw(f, &mut a)).unwrap();
        assert!(
            frame
                .buffer
                .content
                .iter()
                .all(|c| c.diff_option == CellDiffOption::AlwaysUpdate)
        );
    }

    #[test]
    fn scrolls_to_keep_cursor_visible() {
        let text: String = (0..20).map(|i| format!("line {i}\n")).collect();
        let mut a = app(&text);
        a.view.editor.row = 10;
        a.view.editor.col = 3;
        let (rows, cursor) = screen(&mut a, 20, 5);
        // 4 body rows + status bar; cursor row sits at the bottom of the body.
        assert_eq!(rows[3], "line 10");
        assert_eq!(cursor, Position::new(3, 3));
    }

    /// Drives the real crossterm backend into an in-memory VT100 emulator and
    /// checks that what a terminal would display matches what was drawn,
    /// frame after frame, while the cursor walks the whole document.
    #[test]
    fn terminal_output_matches_buffer_while_scrolling() {
        use ratatui::backend::CrosstermBackend;
        use ratatui::layout::Rect;
        use ratatui::{TerminalOptions, Viewport};
        use std::cell::RefCell;
        use std::io::Write;
        use std::rc::Rc;

        #[derive(Clone, Default)]
        struct Sink(Rc<RefCell<Vec<u8>>>);
        impl Write for Sink {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.borrow_mut().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let text = include_str!("../../example_mds/daily_note.md");
        let (w, h) = (125u16, 30u16);
        let sink = Sink::default();
        let mut term = Terminal::with_options(
            CrosstermBackend::new(sink.clone()),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, w, h)),
            },
        )
        .unwrap();
        let mut vt = vt100::Parser::new(h, w, 0);
        let mut a = app(text);
        let n = a.view.editor.lines.len();
        let path: Vec<usize> = (0..n).chain((0..n).rev()).collect();

        for (step, &row) in path.iter().enumerate() {
            a.view.editor.row = row;
            a.view.editor.col = 0;
            let frame = term.draw(|f| draw(f, &mut a)).unwrap();
            let buf = frame.buffer.clone();
            vt.process(&std::mem::take(&mut *sink.0.borrow_mut()));

            for y in 0..h {
                let mut want = String::new();
                let mut x = 0;
                while x < w {
                    let sym = buf[(x, y)].symbol();
                    want.push_str(sym);
                    x += (sym.width() as u16).max(1);
                }
                let got: String = vt.screen().rows(0, w).nth(y as usize).unwrap();
                assert_eq!(
                    got.trim_end(),
                    want.trim_end(),
                    "step {step} (cursor on line {row}), screen row {y}"
                );
            }
        }
    }
}
