//! One open document in an editor: its text, cursor, undo history, folds,
//! scroll position and prompts (search, replace, emoji). This is the part a
//! host embeds, one per tab or pane. Keys go to [`EditorView::handle_key`],
//! which edits the document itself and returns an [`Outcome`] for what only
//! the host can do (open another file, save under a new name, close).
//! Drawing is [`crate::ui::EditorWidget`].

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::text::Line;

use crate::blocks::{FoldKind, analyze_cached};
use crate::editor::Editor;
use crate::emoji::Picker;
use crate::history::Kind;
use crate::links::{self, Link};
use crate::search::{self, Replace, Search};
use crate::shared::Shared;
use crate::ui;
use crate::wrap::LineAttr;

/// What the view is showing besides the text.
#[derive(Debug, Default)]
pub enum ViewMode {
    #[default]
    Edit,
    /// The emoji picker (T-14).
    EmojiPicker(Picker),
    /// The search prompt (V-16).
    Search(Search),
    /// The find-and-replace prompt (V-17).
    Replace(Replace),
}

/// What a key asks of the host, after the view did its part.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// The view handled the key (or it did nothing here).
    Consumed,
    /// The view doesn't use this key: the host's own keymap can.
    Ignored,
    /// Ctrl+Enter on a link to another file (resolved, known to exist).
    OpenLink {
        path: PathBuf,
        heading: Option<String>,
    },
    /// Ctrl+O: open another file.
    RequestOpen,
    /// Ctrl+Shift+S / Ctrl+Alt+S, or Ctrl+S on an untitled document: choose
    /// a file name, then call [`EditorView::save_as`].
    RequestSaveAs,
    /// Ctrl+X: close this document (check [`EditorView::is_dirty`] first).
    RequestClose,
    /// View mode: Enter or a click on a row that a code block processor
    /// gave this action ([`crate::processor::CodeBlockProcessor::render_rows`]).
    Action(String),
    /// A web link (`https://…`, `mailto:` …) was followed: the host may
    /// open it (a browser). mdedit itself only says so.
    OpenUrl(String),
    /// A link to a file that isn't there was followed (`target` as
    /// written): the host may offer to create it. The status says it's
    /// missing.
    MissingLink {
        target: String,
        heading: Option<String>,
    },
}

pub struct EditorView {
    pub editor: Editor,
    /// The document's file; `None` for an untitled document.
    pub path: Option<PathBuf>,
    /// What a document without a file is called (a host's help page, a
    /// diff): its title and status line use it instead of "untitled".
    pub name: Option<String>,
    /// A column the host fills left of the text (a Git change mark, a line's
    /// author), `margin_width` wide (0: none): what to show beside each
    /// line, on its first screen row.
    pub margin: BTreeMap<usize, Line<'static>>,
    pub margin_width: u16,
    pub mode: ViewMode,
    /// Fold states set with Ctrl+K, by header line: `true` = collapsed.
    /// Lines not in here use their default (`-` collapsed, `+` open).
    pub folds: HashMap<usize, bool>,
    /// The last search query, for F3 / Shift+F3 (V-16).
    pub last_search: String,
    /// Source mode (V-13, Ctrl+V): every line shown raw.
    pub source_mode: bool,
    /// First line shown.
    pub scroll: usize,
    /// Rows of the first line scrolled past (a tall rendered block, an
    /// image), while the view is scrolled freely.
    pub scroll_row: usize,
    /// The view was scrolled by [`EditorView::scroll_rows`] (the mouse
    /// wheel): it stays where it was put, not where the cursor is, until
    /// the next key, paste or click.
    pub free_scroll: bool,
    /// Rows to scroll at the next draw (it knows the rows' heights).
    pending_scroll: isize,
    /// The message for the status line (cleared on the next key).
    pub status: String,
    /// Width of the text area in columns, for soft wrap and vertical
    /// movement by screen row. `0` (before the first draw) means no wrap.
    pub width: usize,
    /// Height of the text area in rows (for Page Up / Page Down).
    pub height: usize,
    /// Line size of each row in the last frame (R-16), for the terminal.
    pub line_attrs: Vec<LineAttr>,
    /// Where the cursor was drawn in the last frame.
    pub cursor: Option<Position>,
    /// Screen areas covered by images in the last frame.
    pub image_areas: Vec<Rect>,
    /// View mode (V-12): read-only, every line rendered (the cursor line
    /// too); the cursor moves over rendered rows (embeds and code block
    /// results included), Tab goes from link to link, Enter follows.
    pub reading: bool,
    /// View mode: which rendered row of the cursor's line the cursor is on.
    pub read_row: usize,
    /// View mode: the link (or action) Tab focused on that row.
    pub read_focus: Option<usize>,
    /// The line and rendered row on each screen row in the last frame, and
    /// the text area they were drawn in (for [`EditorView::click`]).
    pub screen_rows: Vec<(usize, usize)>,
    pub text_area: Rect,
    /// The lines as the last key left them (and the document's id), for
    /// the undo history: a key compares against them instead of copying
    /// the whole document first.
    shadow: Vec<String>,
    shadow_id: Option<u64>,
}

impl EditorView {
    /// A view of a document with `text`, saved to `path` (`None`: untitled).
    /// An untitled document ends with a newline when saved, like a new
    /// file; a file keeps its own line endings (F-07).
    pub fn new(text: &str, path: Option<PathBuf>) -> Self {
        let mut editor = Editor::from_text(text);
        if path.is_none() {
            editor.final_newline = true;
        }
        EditorView {
            editor,
            path,
            name: None,
            margin: BTreeMap::new(),
            margin_width: 0,
            mode: ViewMode::Edit,
            folds: HashMap::new(),
            last_search: String::new(),
            source_mode: false,
            scroll: 0,
            scroll_row: 0,
            free_scroll: false,
            pending_scroll: 0,
            status: String::new(),
            width: 0,
            height: 0,
            line_attrs: Vec::new(),
            cursor: None,
            image_areas: Vec::new(),
            reading: false,
            read_row: 0,
            read_focus: None,
            screen_rows: Vec::new(),
            text_area: Rect::default(),
            shadow: Vec::new(),
            shadow_id: None,
        }
    }

    /// A view of the file at `path`. `Err` explains why it couldn't be
    /// read as UTF-8 text.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| String::from_utf8(bytes).map_err(|_| "not UTF-8 text".to_string()))
            .map_err(|e| format!("Cannot open {name}: {e}"))?;
        let mut view = EditorView::new(&text, Some(path.clone()));
        view.status = format!("Opened {}", path.display());
        Ok(view)
    }

    /// Whether the document has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.editor.dirty
    }

    /// The document's name for a tab or title: the file name, else its
    /// [`EditorView::name`], else `untitled`.
    pub fn title(&self) -> String {
        match (self.path.as_deref().and_then(Path::file_name), &self.name) {
            (Some(file), _) => file.to_string_lossy().to_string(),
            (None, Some(name)) => name.clone(),
            (None, None) => "untitled".into(),
        }
    }

    /// The status line: mode, file, unsaved mark, cursor position, the
    /// latest message, then `hints` (if any; they're cut off first on a
    /// narrow screen).
    pub fn status_line(&self, hints: &str) -> String {
        let name = match (&self.path, &self.name) {
            (Some(p), _) => p.display().to_string(),
            (None, Some(name)) => name.clone(),
            (None, None) => "[no file]".into(),
        };
        let dirty = if self.is_dirty() { " [+]" } else { "" };
        let mode = if self.source_mode {
            " SOURCE │"
        } else if self.reading {
            " VIEW │"
        } else {
            ""
        };
        let (row, col) = (self.editor.row + 1, self.editor.col + 1);
        let message = if self.status.is_empty() {
            String::new()
        } else {
            format!(" {} │", self.status)
        };
        format!("{mode} {name}{dirty}  Ln {row}, Col {col}  │{message} {hints}")
            .trim_end()
            .to_string()
    }

    /// Writes the document to its file (F-01, F-07). `Err` if it's untitled
    /// or the write failed; the status line says what happened.
    pub fn save(&mut self) -> Result<(), String> {
        let Some(path) = self.path.clone() else {
            return Err("The document has no file name yet".into());
        };
        self.save_as(path)
    }

    /// Writes the document to `path`, which becomes its file (F-02).
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        match crate::files::write_atomic(&path, &self.editor.to_text()) {
            Ok(()) => {
                self.editor.mark_saved();
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                Ok(())
            }
            Err(e) => {
                let message = format!("Save failed: {e}");
                self.status = message.clone();
                Err(message)
            }
        }
    }

    /// Scrolls the view by `rows` screen rows (down if positive), like a
    /// mouse wheel: through tall rendered blocks too, without moving the
    /// cursor (which would show a block's source). The next key, paste or
    /// click goes back to the cursor. Not for view mode, where the row
    /// cursor moves instead (send Up / Down).
    pub fn scroll_rows(&mut self, rows: isize) {
        self.free_scroll = true;
        self.pending_scroll += rows;
    }

    /// Takes the rows to scroll at this draw.
    pub(crate) fn take_pending_scroll(&mut self) -> isize {
        std::mem::take(&mut self.pending_scroll)
    }

    /// Back to the cursor (after a key, a paste or a click).
    fn follow_cursor(&mut self) {
        if self.free_scroll {
            self.free_scroll = false;
            self.pending_scroll = 0;
            self.scroll_row = 0;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, shared: &mut Shared) -> Outcome {
        self.follow_cursor();
        self.status.clear();
        self.editor.indent_width = shared.config.indent_width;
        if matches!(self.mode, ViewMode::Edit) && !self.reading && is_paste_key(&key) {
            // Like a pasted text: one undo step; a URL over a selection links it.
            match shared.clipboard.read() {
                Some(text) => self.handle_paste(&text, shared),
                None => {
                    self.status =
                        "Nothing to paste: the clipboard is empty or can't be read (install wl-clipboard or xclip)"
                            .into();
                }
            }
            return Outcome::Consumed;
        }
        if matches!(self.mode, ViewMode::Edit)
            && !self.reading
            && let Some(redo) = undo_key(&key)
        {
            self.undo(redo);
            return Outcome::Consumed;
        }
        let kind = match (&self.mode, key.code) {
            _ if key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Kind::Other
            }
            (ViewMode::Edit, KeyCode::Char(c)) => Kind::Typing(c.is_whitespace()),
            (ViewMode::Edit, KeyCode::Backspace | KeyCode::Delete) => Kind::Deleting,
            _ => Kind::Other,
        };
        let before = self.editor_state();
        let outcome = self.dispatch_key(key, shared);
        self.record(before, kind, shared);
        outcome
    }

    /// Pasted text (bracketed paste): goes in as typed text, never as keys,
    /// so newlines don't continue lists and tabs don't indent.
    pub fn handle_paste(&mut self, text: &str, shared: &mut Shared) {
        self.follow_cursor();
        if self.reading && matches!(self.mode, ViewMode::Edit) {
            self.status = READ_ONLY.into();
            return;
        }
        let before = self.editor_state();
        self.paste(text, shared);
        self.record(before, Kind::Other, shared);
    }

    /// The document and cursor before a key, for [`EditorView::record`].
    /// The lines before it are the shadow, brought up to date first if
    /// something else changed them (a host, an undo): comparing costs no
    /// copies, and copying happens only then.
    fn editor_state(&mut self) -> (u64, (usize, usize)) {
        let ed = &self.editor;
        if self.shadow_id != Some(ed.id) || self.shadow != ed.lines {
            self.shadow = ed.lines.clone();
            self.shadow_id = Some(ed.id);
        }
        (ed.id, (ed.row, ed.col))
    }

    /// Adds the change a key or paste made (if any) to the undo history,
    /// and brings the shadow up to date (the changed lines only).
    fn record(&mut self, (id, cursor): (u64, (usize, usize)), kind: Kind, shared: &Shared) {
        let ed = &mut self.editor;
        ed.history.limit = shared.config.undo_steps;
        if ed.id != id {
            return; // Another document: the next key takes a new shadow.
        }
        let after = (ed.row, ed.col);
        ed.history
            .record(&self.shadow, &ed.lines, (cursor, after), kind);
        let (old, new) = (&mut self.shadow, &ed.lines);
        let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        let room = old.len().min(new.len()) - prefix;
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        old.splice(
            prefix..old.len() - suffix,
            new[prefix..new.len() - suffix].iter().cloned(),
        );
    }

    /// Ctrl+Z / Ctrl+Y (V-18).
    fn undo(&mut self, redo: bool) {
        let lines_before = self.editor.lines.len();
        if !self.editor.undo(redo) {
            self.status = format!("Nothing to {}", if redo { "redo" } else { "undo" });
            return;
        }
        if self.editor.lines.len() != lines_before {
            self.folds.clear();
        }
        self.reveal_cursor();
    }

    fn dispatch_key(&mut self, key: KeyEvent, shared: &mut Shared) -> Outcome {
        match std::mem::take(&mut self.mode) {
            ViewMode::Edit if self.reading => self.key_reading(key, shared),
            ViewMode::Edit => {
                let lines_before = self.editor.lines.len();
                let outcome = self.key_edit(key, shared);
                // Fold choices are by line number: reset them when lines
                // are added or removed.
                if self.editor.lines.len() != lines_before {
                    self.folds.clear();
                }
                self.reveal_cursor();
                outcome
            }
            ViewMode::EmojiPicker(p) => self.key_picker(key, p, shared),
            ViewMode::Search(search) => self.key_search(key, search),
            ViewMode::Replace(r) => self.key_replace(key, r),
        }
    }

    fn paste(&mut self, text: &str, shared: &mut Shared) {
        self.status.clear();
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        match &self.mode {
            ViewMode::Edit => {
                let lines_before = self.editor.lines.len();
                let selected = self.editor.selected_text();
                self.editor.delete_selection();
                match selected {
                    // V-09: a URL pasted over a selection links it.
                    Some(label) if is_url(&text) => {
                        self.editor.paste(&format!("[{label}]({})", text.trim()));
                    }
                    _ => self.editor.paste(&text),
                }
                if self.editor.lines.len() != lines_before {
                    self.folds.clear();
                }
                self.reveal_cursor();
            }
            // Text fields take the first line, as if typed.
            ViewMode::Search(_) | ViewMode::Replace(_) | ViewMode::EmojiPicker(_) => {
                let first = text.lines().next().unwrap_or_default();
                for c in first.chars().filter(|c| !c.is_control()) {
                    self.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), shared);
                }
            }
        }
    }

    fn key_edit(&mut self, key: KeyEvent, shared: &mut Shared) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        if ctrl && is_save_as_key(key.code, alt, shift) {
            return Outcome::RequestSaveAs;
        }
        if key.code == KeyCode::Enter && (ctrl || alt) {
            return self.follow_link(shared);
        }
        // Selection (V-19): Shift + a move extends it, a plain move ends it.
        let movement = !ctrl
            && !alt
            && matches!(
                key.code,
                KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
            );
        if movement {
            self.editor.select_with(shift);
        } else if ctrl && key.code == KeyCode::Char('a') {
            self.editor.select_all();
            return Outcome::Consumed;
        } else if self.editor.selection().is_some() && self.key_selection(key) {
            return Outcome::Consumed;
        }
        let outcome = self.key_edit_text(key, shared);
        if !movement {
            self.editor.anchor = None;
        }
        outcome
    }

    /// A key that acts on the selection instead of at the cursor. Returns
    /// false if `key` doesn't (then the selection ends and the key does
    /// what it always does, after typing or Enter deleted the selection).
    fn key_selection(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let ed = &mut self.editor;
        match key.code {
            KeyCode::Char(c) if !ctrl => match wrap_close(c) {
                // V-05: `*` wraps the selection in `*` … `*`.
                Some(close) => {
                    ed.wrap_selection(&c.to_string(), close);
                    true
                }
                None => {
                    ed.delete_selection();
                    false
                }
            },
            KeyCode::Enter => {
                ed.delete_selection();
                false
            }
            KeyCode::Backspace | KeyCode::Delete => {
                ed.delete_selection();
                true
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let ((r0, _), (r1, _)) = ed.selection().expect("checked by the caller");
                for row in r0..=r1 {
                    (ed.row, ed.col) = (row, 0);
                    if key.code == KeyCode::Tab {
                        ed.indent();
                    } else {
                        ed.outdent();
                    }
                }
                // The whole lines stay selected.
                ed.anchor = Some((r0, 0));
                ed.col = ed.lines[r1].chars().count();
                true
            }
            _ => false,
        }
    }

    /// Keys in the editor other than cursor movement with Shift.
    fn key_edit_text(&mut self, key: KeyEvent, shared: &mut Shared) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let auto_pair = shared.config.auto_pair;
        let ed = &mut self.editor;

        match key.code {
            // Ctrl+R too: some terminals send Ctrl+H for Backspace.
            KeyCode::Char('h' | 'r') if ctrl => {
                self.mode = ViewMode::Replace(Replace {
                    find: String::new(),
                    with: String::new(),
                    editing_with: false,
                    origin: (ed.row, ed.col),
                });
            }
            KeyCode::Char('f') if ctrl => {
                self.mode = ViewMode::Search(Search {
                    query: String::new(),
                    origin: (ed.row, ed.col),
                });
            }
            KeyCode::F(3) => self.search_step(!shift),
            // Alt+V: live preview → source → view (Ctrl+Shift+V stays the
            // terminal's paste).
            KeyCode::Char('v' | 'V') if key.modifiers == KeyModifiers::ALT => self.cycle_mode(),
            KeyCode::Char('k') if ctrl => self.toggle_fold(),
            KeyCode::Char('e') if ctrl => {
                self.mode = ViewMode::EmojiPicker(Picker::with_recent(&shared.recent.items));
            }
            KeyCode::Char('o') if ctrl => return Outcome::RequestOpen,
            KeyCode::Char('x') if ctrl => return Outcome::RequestClose,
            KeyCode::Char('s') if ctrl => {
                if self.path.is_none() {
                    return Outcome::RequestSaveAs;
                }
                // The status line says whether it worked.
                let _ = self.save();
            }
            KeyCode::Char('t') if ctrl => ed.create_task(),
            KeyCode::Char('l') if ctrl => {
                if !ed.toggle_task() {
                    self.status = "Not a task line".into();
                }
            }
            KeyCode::Char(c) if !ctrl && auto_pair => ed.type_char(c),
            KeyCode::Char(c) if !ctrl => ed.insert_char(c),
            KeyCode::Enter => ed.newline(),
            KeyCode::Backspace if auto_pair => ed.backspace_pair(),
            KeyCode::Backspace => ed.backspace(),
            KeyCode::Delete => ed.delete(),
            KeyCode::Tab => ed.indent(),
            KeyCode::BackTab => ed.outdent(),
            KeyCode::Left => ed.left(),
            KeyCode::Right => ed.right(),
            KeyCode::PageUp => self.page(false),
            KeyCode::PageDown => self.page(true),
            KeyCode::Up => self.vertical(false),
            KeyCode::Down => self.vertical(true),
            KeyCode::Home => ed.home(),
            KeyCode::End => ed.end(),
            // Esc ends a selection (done by the caller).
            KeyCode::Esc => {}
            _ => return Outcome::Ignored,
        }
        Outcome::Consumed
    }

    /// Emoji picker (T-14): typing searches, arrows move through the grid,
    /// Enter inserts the highlighted emoji at the cursor, Esc or Ctrl+E
    /// cancels.
    fn key_picker(&mut self, key: KeyEvent, mut p: Picker, shared: &mut Shared) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => return Outcome::Consumed, // mode is already Edit
            KeyCode::Char('e') if ctrl => return Outcome::Consumed,
            KeyCode::Enter => {
                if let Some(e) = p.current() {
                    self.editor.insert_str(e.as_str());
                    shared.recent.add(e);
                    if let Err(err) = shared.recent.save() {
                        self.status = format!("Could not save recent emoji: {err}");
                    }
                    return Outcome::Consumed; // mode is already Edit
                }
            }
            KeyCode::Left => p.move_by(-1, 0),
            KeyCode::Right => p.move_by(1, 0),
            KeyCode::Up => p.move_by(0, -1),
            KeyCode::Down => p.move_by(0, 1),
            KeyCode::Backspace => {
                let mut q = std::mem::take(&mut p.query);
                q.pop();
                p.set_query(q);
            }
            KeyCode::Char(c) if !ctrl => {
                let q = format!("{}{c}", p.query);
                p.set_query(q);
            }
            _ => {}
        }
        self.mode = ViewMode::EmojiPicker(p);
        Outcome::Consumed
    }

    /// The search prompt (V-16): typing jumps to the first match from where
    /// the search started, ↓ / F3 and ↑ / Shift+F3 move between matches,
    /// Enter keeps the position, Esc goes back.
    fn key_search(&mut self, key: KeyEvent, mut s: Search) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Esc => {
                (self.editor.row, self.editor.col) = s.origin;
                return Outcome::Consumed; // mode is already Edit
            }
            KeyCode::Enter => return Outcome::Consumed,
            KeyCode::Down => self.search_step(true),
            KeyCode::Up => self.search_step(false),
            KeyCode::F(3) => self.search_step(!shift),
            KeyCode::Backspace => {
                s.query.pop();
                self.search_from_origin(&s);
            }
            KeyCode::Char(c) if !ctrl => {
                s.query.push(c);
                self.search_from_origin(&s);
            }
            _ => {}
        }
        self.mode = ViewMode::Search(s);
        Outcome::Consumed
    }

    /// A key in the find-and-replace prompt (V-17). Typing in the find
    /// field jumps to the first match like search; Enter replaces the match
    /// under the cursor and goes to the next, Ctrl+A replaces them all.
    fn key_replace(&mut self, key: KeyEvent, mut r: Replace) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Esc => return Outcome::Consumed, // mode is already Edit
            KeyCode::Tab | KeyCode::BackTab => r.editing_with = !r.editing_with,
            KeyCode::Down => self.search_step(true),
            KeyCode::Up => self.search_step(false),
            KeyCode::F(3) => self.search_step(!shift),
            KeyCode::Enter => self.replace_next(&r),
            KeyCode::Char('a') if ctrl => {
                let n = search::replace_all(&mut self.editor.lines, &r.find, &r.with);
                if n > 0 {
                    self.editor.dirty = true;
                    self.editor.snap_col();
                }
                let s = if n == 1 { "" } else { "es" };
                self.status = format!("Replaced {n} match{s}");
                return Outcome::Consumed;
            }
            KeyCode::Backspace if r.editing_with => {
                r.with.pop();
            }
            KeyCode::Char(c) if !ctrl && r.editing_with => r.with.push(c),
            KeyCode::Backspace => {
                r.find.pop();
                self.find_from_origin(&r);
            }
            KeyCode::Char(c) if !ctrl => {
                r.find.push(c);
                self.find_from_origin(&r);
            }
            _ => {}
        }
        self.mode = ViewMode::Replace(r);
        Outcome::Consumed
    }

    /// Enter in the replace prompt: replaces the match at the cursor (if
    /// the cursor is on one), then moves to the next match.
    fn replace_next(&mut self, r: &Replace) {
        let at = (self.editor.row, self.editor.col);
        let mut from = at;
        if let Some(after) = search::replace_at(&mut self.editor.lines, &r.find, at, &r.with) {
            self.editor.dirty = true;
            from = after;
        }
        match search::find(&self.editor.lines, &r.find, from, true, true) {
            Some(found) => self.land_on_match(found),
            None => {
                (self.editor.row, self.editor.col) = from;
                self.editor.snap_col();
                self.status = "No match".into();
            }
        }
    }

    fn find_from_origin(&mut self, r: &Replace) {
        let s = Search {
            query: r.find.clone(),
            origin: r.origin,
        };
        self.search_from_origin(&s);
    }

    /// Jumps to the first match at or after where the search started.
    fn search_from_origin(&mut self, s: &Search) {
        self.last_search = s.query.clone();
        let found = search::find(&self.editor.lines, &s.query, s.origin, true, true);
        self.land_on_match(found.unwrap_or(s.origin));
    }

    /// F3 / Shift+F3 (and ↓ / ↑ in the prompt): the next or previous match
    /// of the last search, wrapping around.
    fn search_step(&mut self, forward: bool) {
        if self.last_search.is_empty() {
            self.status = "Nothing to search for: press Ctrl+F".into();
            return;
        }
        let from = (self.editor.row, self.editor.col);
        match search::find(&self.editor.lines, &self.last_search, from, forward, false) {
            Some(found) => self.land_on_match(found),
            None => self.status = "No match".into(),
        }
    }

    /// Moves the cursor to `at`, opens any fold hiding it, and reports the
    /// match count.
    fn land_on_match(&mut self, at: (usize, usize)) {
        (self.editor.row, self.editor.col) = at;
        self.reveal_cursor();
        self.status = search::describe(&self.editor.lines, &self.last_search, at);
    }

    /// Ctrl+K (B-05, V-06, V-07): folds or unfolds the innermost section,
    /// list item or callout around the cursor. When folding from inside,
    /// the cursor moves to the header line.
    fn toggle_fold(&mut self) {
        let structure = analyze_cached(&self.editor.lines);
        let Some(fold) = structure.fold_around(self.editor.row) else {
            self.status =
                "Nothing to fold here (headings, list items with sub-items, callouts)".into();
            return;
        };
        let collapse = !structure.is_collapsed(fold, &self.folds);
        self.folds.insert(fold.header, collapse);
        let what = match fold.kind {
            FoldKind::Section => "section",
            FoldKind::List => "list item",
            FoldKind::Callout => "callout",
        };
        let n = fold.end + 1 - fold.start;
        let lines = if n == 1 { "line" } else { "lines" };
        let verb = if collapse { "Folded" } else { "Unfolded" };
        self.status = format!("{verb} {what} ({n} {lines})");
        if collapse && self.editor.row > fold.header {
            self.editor.row = fold.header;
            self.editor.col = 0;
        }
    }

    /// Opens any collapsed fold that hides the cursor's line, so text is
    /// never edited while hidden.
    fn reveal_cursor(&mut self) {
        if self.source_mode {
            return; // nothing is hidden in source mode
        }
        let structure = analyze_cached(&self.editor.lines);
        let hiding: Vec<usize> = structure
            .hiding(self.editor.row, &self.folds)
            .map(|f| f.header)
            .collect();
        for header in hiding {
            self.folds.insert(header, false);
        }
    }

    /// Page Up / Page Down (V-15): moves the cursor a screen of rows (less
    /// one, for context) and scrolls the view by the lines it crossed.
    fn page(&mut self, down: bool) {
        let rows = self.height.saturating_sub(1).max(1);
        let start = self.editor.row;
        for _ in 0..rows {
            let before = (self.editor.row, self.editor.col);
            self.vertical(down);
            if (self.editor.row, self.editor.col) == before {
                break;
            }
        }
        let crossed = self.editor.row.abs_diff(start);
        self.scroll = if down {
            self.scroll + crossed
        } else {
            self.scroll.saturating_sub(crossed)
        };
    }

    /// Moves the cursor one screen row up or down. Long lines are soft-wrapped
    /// (see [`crate::wrap`]), so this moves within a line before moving to the
    /// next one, keeping the screen column.
    fn vertical(&mut self, down: bool) {
        let width = self.width;
        let ed = &mut self.editor;
        if width == 0 {
            // No layout yet (tests): line by line, skipping hidden lines.
            let structure = analyze_cached(&ed.lines);
            let hidden =
                |r: usize| !self.source_mode && structure.hiding(r, &self.folds).next().is_some();
            let row = ed.row;
            let target = if down {
                (row + 1..ed.lines.len()).find(|&r| !hidden(r))
            } else {
                (0..row).rev().find(|&r| !hidden(r))
            };
            if let Some(r) = target {
                ed.row = r;
                ed.col = ed.col.min(ed.lines[r].chars().count());
                ed.snap_col();
            }
            return;
        }
        let layout = ui::raw_layout(&ed.lines[ed.row], width);
        let (row, x) = layout.position(ui::display_offset(&ed.lines[ed.row], ed.col));
        let last = layout.starts.len() - 1;
        let (line, target_row) = match (down, row) {
            (true, r) if r < last => (ed.row, r + 1),
            (false, r) if r > 0 => (ed.row, r - 1),
            (true, _) if ed.row + 1 < ed.lines.len() => (ed.row + 1, 0),
            (false, _) if ed.row > 0 => {
                let prev = ui::raw_layout(&ed.lines[ed.row - 1], width);
                (ed.row - 1, prev.starts.len() - 1)
            }
            _ => return,
        };
        // Skip lines hidden in collapsed folds.
        let structure = analyze_cached(&ed.lines);
        let hidden =
            |r: usize| !self.source_mode && structure.hiding(r, &self.folds).next().is_some();
        let (line, target_row) = if !hidden(line) {
            (line, target_row)
        } else if down {
            match (line..ed.lines.len()).find(|&r| !hidden(r)) {
                Some(r) => (r, 0),
                None => return,
            }
        } else {
            let r = (0..line).rev().find(|&r| !hidden(r)).unwrap_or(0);
            (r, ui::raw_layout(&ed.lines[r], width).starts.len() - 1)
        };
        let text = &ed.lines[line];
        let offset = ui::raw_layout(text, width).offset_at(target_row, x);
        ed.row = line;
        ed.col = ui::col_at_offset(text, offset);
    }

    /// Ctrl+Enter / Alt+Enter (F-05): the link under the cursor. A heading
    /// in this note (`[[#Heading]]`, or a link to this very file) is jumped
    /// to here; another file is [`Outcome::OpenLink`] for the host.
    fn follow_link(&mut self, shared: &Shared) -> Outcome {
        let line = &self.editor.lines[self.editor.row];
        // A reference link (K-07) points through its definition.
        let link = links::link_at(line, self.editor.col).or_else(|| {
            links::reference_at(line, self.editor.col)
                .and_then(|label| links::find_definition(&self.editor.lines, &label))
        });
        self.follow(link, shared)
    }

    /// Follows `link`: see [`EditorView::follow_link`].
    fn follow(&mut self, link: Option<Link>, shared: &Shared) -> Outcome {
        let (target, heading) = match link {
            None => {
                self.status = "No link under the cursor".into();
                return Outcome::Consumed;
            }
            Some(Link::Web(url)) => return Outcome::OpenUrl(url),
            Some(Link::File { path, heading }) => (path, heading),
        };
        if target.is_empty() {
            if let Some(heading) = heading {
                self.go_to_heading(&heading);
            }
            return Outcome::Consumed;
        }
        match shared.resolver.resolve(self.path.as_deref(), &target) {
            Err(missing) => {
                self.status = format!("Not found: {missing}");
                Outcome::MissingLink { target, heading }
            }
            // A link to this note (`[[this#Heading]]`) jumps within it, keeping
            // unsaved changes.
            Ok(path) if self.is_this_file(&path) => {
                match heading {
                    Some(heading) => {
                        self.go_to_heading(&heading);
                    }
                    None => self.status = "That link points to this note".into(),
                }
                Outcome::Consumed
            }
            Ok(path) => Outcome::OpenLink { path, heading },
        }
    }

    /// Alt+V: live preview → source → view → live preview.
    fn cycle_mode(&mut self) {
        if self.source_mode {
            self.source_mode = false;
            self.enter_reading();
        } else if self.reading {
            self.reading = false;
            self.status = "Live preview".into();
        } else {
            self.source_mode = true;
            self.status = "Source mode: every line raw (Alt+V: view mode)".into();
        }
    }

    /// Turns view mode on (V-12), with the row cursor on the cursor's line.
    pub fn enter_reading(&mut self) {
        self.source_mode = false;
        self.reading = true;
        self.read_row = 0;
        self.read_focus = None;
        self.editor.anchor = None;
        self.status =
            "View mode: read-only; Tab / Enter follow links, Esc edits (Alt+V: live preview)"
                .into();
    }

    /// Keys in view mode: moving, Tab / Shift+Tab to a link or action,
    /// Enter (or Ctrl+Enter) to follow it, Esc to edit again. The keys
    /// that change nothing still work (open, close, save as, search, fold,
    /// the mode keys); the rest say it's read-only.
    fn key_reading(&mut self, key: KeyEvent, shared: &mut Shared) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let page = self.height.max(2) as isize - 1;
        match key.code {
            KeyCode::Esc => {
                self.reading = false;
                self.status = "Live preview".into();
            }
            KeyCode::Up => self.read_move(-1, shared),
            KeyCode::Down => self.read_move(1, shared),
            KeyCode::PageUp => self.read_move(-page, shared),
            KeyCode::PageDown => self.read_move(page, shared),
            KeyCode::Home if ctrl => self.read_move(isize::MIN / 2, shared),
            KeyCode::End if ctrl => self.read_move(isize::MAX / 2, shared),
            KeyCode::Tab => self.read_tab(true, shared),
            KeyCode::BackTab => self.read_tab(false, shared),
            KeyCode::Enter => return self.read_activate(shared),
            KeyCode::Char('o' | 'x' | 'f' | 'k' | 's') if ctrl => {
                return self.key_edit(key, shared);
            }
            KeyCode::Char('s' | 'S') if ctrl && (alt || shift) => {
                return self.key_edit(key, shared);
            }
            KeyCode::Char('v' | 'V') if alt && !ctrl => return self.key_edit(key, shared),
            KeyCode::F(3) => return self.key_edit(key, shared),
            _ => self.status = READ_ONLY.into(),
        }
        Outcome::Consumed
    }

    /// The rendered rows of line `i` in view mode.
    fn rows_of(&self, i: usize, shared: &Shared) -> Vec<ui::ViewRow> {
        ui::view_rows(self, shared, i)
    }

    /// Moves the row cursor `delta` rendered rows (lines with no rows, like
    /// folded ones, are skipped).
    fn read_move(&mut self, delta: isize, shared: &Shared) {
        self.read_focus = None;
        let last = self.editor.lines.len() - 1;
        let mut count = self.rows_of(self.editor.row, shared).len();
        for _ in 0..delta.unsigned_abs() {
            if delta > 0 {
                if self.read_row + 1 < count {
                    self.read_row += 1;
                    continue;
                }
                let next = (self.editor.row + 1..=last).find_map(|r| {
                    let n = self.rows_of(r, shared).len();
                    (n > 0).then_some((r, n))
                });
                let Some((r, n)) = next else { break };
                (self.editor.row, self.read_row, count) = (r, 0, n);
            } else {
                if self.read_row > 0 {
                    self.read_row -= 1;
                    continue;
                }
                let prev = (0..self.editor.row).rev().find_map(|r| {
                    let n = self.rows_of(r, shared).len();
                    (n > 0).then_some((r, n))
                });
                let Some((r, n)) = prev else { break };
                (self.editor.row, self.read_row, count) = (r, n - 1, n);
            }
        }
        self.editor.col = 0;
    }

    /// Tab / Shift+Tab: the next (previous) link or action in the text.
    fn read_tab(&mut self, forward: bool, shared: &Shared) {
        let (row, sub) = (self.editor.row, self.read_row);
        let mut lines: Box<dyn Iterator<Item = usize>> = if forward {
            Box::new(row..self.editor.lines.len())
        } else {
            Box::new((0..=row).rev())
        };
        let found = lines.find_map(|r| {
            let rows = self.rows_of(r, shared);
            let mut spots: Vec<(usize, usize)> = rows
                .iter()
                .enumerate()
                .flat_map(|(s, vr)| (0..vr.items.len()).map(move |i| (s, i)))
                .collect();
            if !forward {
                spots.reverse();
            }
            spots
                .into_iter()
                .find(|&(s, i)| {
                    if r != row {
                        return true;
                    }
                    let here = (s, i);
                    let at = (sub, self.read_focus);
                    match (forward, at.1) {
                        (true, Some(f)) => here > (at.0, f),
                        (true, None) => s >= at.0,
                        (false, Some(f)) => here < (at.0, f),
                        (false, None) => s < at.0,
                    }
                })
                .map(|(s, i)| (r, s, i, rows[s].items[i].action.clone()))
        });
        match found {
            Some((r, s, i, action)) => {
                (self.editor.row, self.read_row, self.read_focus) = (r, s, Some(i));
                self.editor.col = 0;
                self.status = match action {
                    ui::RowAction::Link(Link::File { path, heading }) => {
                        let at = heading.map(|h| format!("#{h}")).unwrap_or_default();
                        format!("→ {path}{at} (Enter follows)")
                    }
                    ui::RowAction::Link(Link::Web(url)) => format!("→ {url}"),
                    ui::RowAction::Host(_) => "Enter: open".into(),
                };
            }
            None => {
                self.status = if forward {
                    "No more links below"
                } else {
                    "No more links above"
                }
                .into();
            }
        }
    }

    /// View mode: the host action Enter would give for the row the
    /// cursor is on (its focused item, or its first), without acting;
    /// `None` outside view mode or on a row without one.
    pub fn read_action(&self, shared: &Shared) -> Option<String> {
        if !self.reading {
            return None;
        }
        let rows = self.rows_of(self.editor.row, shared);
        let row = rows.get(self.read_row)?;
        let item = self
            .read_focus
            .and_then(|f| row.items.get(f))
            .or_else(|| row.items.first())?;
        match &item.action {
            ui::RowAction::Host(action) => Some(action.clone()),
            ui::RowAction::Link(_) => None,
        }
    }

    /// Enter: follows the focused link (or the row's first), or gives the
    /// row's action to the host.
    fn read_activate(&mut self, shared: &Shared) -> Outcome {
        let rows = self.rows_of(self.editor.row, shared);
        let Some(row) = rows.get(self.read_row) else {
            return Outcome::Consumed;
        };
        let item = self
            .read_focus
            .and_then(|f| row.items.get(f))
            .or_else(|| row.items.first());
        match item.map(|i| i.action.clone()) {
            Some(ui::RowAction::Host(action)) => Outcome::Action(action),
            Some(ui::RowAction::Link(link)) => self.follow(Some(link), shared),
            None => {
                self.status = "Nothing to follow on this row (Tab: the next link)".into();
                Outcome::Consumed
            }
        }
    }

    /// A click at `at` on the screen. On a link (or an embed, an image, a
    /// code block result row) it follows it, as Enter does in view mode;
    /// in view mode a click elsewhere moves the row cursor there. Anywhere
    /// else in the text it places the cursor: on the line being edited (raw
    /// text) and in source mode at that column, on a rendered line where
    /// the clicked text is in its source. Outside the text it's `Ignored`.
    pub fn click(&mut self, at: Position, shared: &mut Shared) -> Outcome {
        if !self.text_area.contains(at) {
            return Outcome::Ignored;
        }
        let y = usize::from(at.y - self.text_area.y);
        let Some(&(line, sub)) = self.screen_rows.get(y) else {
            return Outcome::Ignored;
        };
        let x = usize::from(at.x - self.text_area.x);
        if self.source_mode || (!self.reading && line == self.editor.row) {
            let text = &self.editor.lines[line];
            let offset = ui::raw_layout(text, self.width).offset_at(sub, x);
            let col = ui::col_at_offset(text, offset);
            self.place_cursor(line, col);
            return Outcome::Consumed;
        }
        let rows = self.rows_of(line, shared);
        let hit = rows
            .get(sub)
            .and_then(|r| r.items.iter().position(|i| (i.from..i.to).contains(&x)));
        if self.reading {
            (self.editor.row, self.read_row, self.editor.col) = (line, sub, 0);
            self.read_focus = hit;
            return match hit {
                Some(_) => self.read_activate(shared),
                None => Outcome::Consumed,
            };
        }
        let Some(item) = hit.and_then(|h| rows[sub].items.get(h)) else {
            let shown: Vec<&str> = rows.iter().map(|r| r.text.as_str()).collect();
            let col = ui::source_col(&self.editor.lines[line], &shown, sub, x);
            self.place_cursor(line, col);
            return Outcome::Consumed;
        };
        match item.action.clone() {
            ui::RowAction::Host(action) => Outcome::Action(action),
            ui::RowAction::Link(link) => self.follow(Some(link), shared),
        }
    }

    /// Puts the cursor at char `col` of `line`, ending a selection.
    fn place_cursor(&mut self, line: usize, col: usize) {
        let ed = &mut self.editor;
        ed.row = line;
        ed.col = col;
        ed.anchor = None;
        ed.snap_col();
    }

    /// Whether `path` is the file being edited.
    fn is_this_file(&self, path: &Path) -> bool {
        let real = |p: &Path| std::fs::canonicalize(p).ok();
        self.path
            .as_deref()
            .is_some_and(|own| real(own).is_some() && real(own) == real(path))
    }

    /// Moves the cursor to the heading `heading` (see
    /// [`links::heading_matches`]) and scrolls it to the top of the screen.
    /// Returns false (with a status message) if there's no such heading.
    pub fn go_to_heading(&mut self, heading: &str) -> bool {
        if let Some(id) = heading.strip_prefix('^') {
            // A block (E-03): its line.
            return match crate::embed::block_line(&self.editor.lines, id) {
                Some(row) => {
                    self.editor.row = row;
                    self.editor.col = 0;
                    self.scroll = row.saturating_sub(3);
                    true
                }
                None => {
                    self.status = format!("Block not found: ^{id}");
                    false
                }
            };
        }
        let structure = analyze_cached(&self.editor.lines);
        let lines = &self.editor.lines;
        let found = (0..lines.len()).find(|&i| {
            structure
                .heading(lines, i)
                .is_some_and(|(_, text)| links::heading_matches(text, heading))
        });
        match found {
            Some(row) => {
                // The heading goes to the top of the screen (F-06).
                self.editor.row = row;
                self.editor.col = 0;
                self.scroll = row;
                true
            }
            None => {
                self.status = format!("Heading not found: {heading}");
                false
            }
        }
    }
}

/// The closing text for a character that wraps a selection (V-05).
fn wrap_close(c: char) -> Option<&'static str> {
    Some(match c {
        '*' => "*",
        '_' => "_",
        '~' => "~",
        '=' => "=",
        '`' => "`",
        '"' => "\"",
        '(' => ")",
        '[' => "]",
        '{' => "}",
        _ => return None,
    })
}

/// Whether pasted `text` is a web address (V-09).
fn is_url(text: &str) -> bool {
    let text = text.trim();
    (text.starts_with("https://") || text.starts_with("http://"))
        && !text.contains(char::is_whitespace)
}

/// Whether `key` is undo (`Some(false)`: Ctrl+Z) or redo (`Some(true)`:
/// Ctrl+Y, or Ctrl+Shift+Z where the terminal reports it).
fn undo_key(key: &KeyEvent) -> Option<bool> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Char('z') if !shift => Some(false),
        KeyCode::Char('Z' | 'z') | KeyCode::Char('y') => Some(true),
        _ => None,
    }
}

/// What view mode says when a key would edit.
const READ_ONLY: &str = "View mode: read-only (Esc to edit)";

/// Paste: Ctrl+V alone (with Shift or Alt it's source mode).
fn is_paste_key(key: &KeyEvent) -> bool {
    // Ctrl+Shift+V too, where the terminal reports it instead of pasting.
    matches!(key.code, KeyCode::Char('v' | 'V'))
        && (key.modifiers == KeyModifiers::CONTROL
            || key.modifiers == KeyModifiers::CONTROL | KeyModifiers::SHIFT)
}

/// Save As: Ctrl+Shift+S where the terminal can report it (kitty keyboard
/// protocol), and Ctrl+Alt+S everywhere (F-02 fallback key).
fn is_save_as_key(code: KeyCode, alt: bool, shift: bool) -> bool {
    match code {
        KeyCode::Char('S') => true,
        KeyCode::Char('s') => alt || shift,
        _ => false,
    }
}
