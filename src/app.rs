//! The mdedit program around one [`EditorView`]: what happens when the
//! view asks to open a file, save under a new name or close (the file
//! browser, the save prompt, following a link in place, exiting). A host
//! that embeds mdedit replaces this with its own tabs and dialogs. Kept
//! apart from the terminal loop so it can be driven from tests.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::files::is_path_like;
pub use crate::files::{After, Browser, Entry, Mode, Purpose};
use crate::shared::Shared;
pub use crate::view::{EditorView, Outcome, ViewMode};

pub struct App {
    /// Settings, terminal, images, emoji and links, shared by all views.
    pub shared: Shared,
    /// The document being edited.
    pub view: EditorView,
    /// Where file dialogs start, and links resolve, for an untitled
    /// document (see [`App::set_cwd`]).
    cwd: PathBuf,
    /// The dialog on top of the view, if any (`Mode::Edit`: none).
    pub mode: Mode,
    /// The file (and heading) to open once the save prompt is answered (F-05).
    pub pending_link: Option<(PathBuf, Option<String>)>,
}

/// What the terminal loop should do after a key was handled.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
}

impl App {
    pub fn new(text: &str, path: Option<PathBuf>) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut shared = Shared::new();
        shared.resolver = Box::new(crate::resolver::RelativeResolver { cwd: cwd.clone() });
        Self {
            shared,
            view: EditorView::new(text, path),
            cwd,
            mode: Mode::Edit,
            pending_link: None,
        }
    }

    /// Sets the folder for an untitled document's file dialogs and links.
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.shared.resolver = Box::new(crate::resolver::RelativeResolver { cwd: cwd.clone() });
        self.cwd = cwd;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        match std::mem::take(&mut self.mode) {
            Mode::Edit => {
                let outcome = self.view.handle_key(key, &mut self.shared);
                self.act_on(outcome)
            }
            dialog => {
                self.view.status.clear();
                match dialog {
                    Mode::Edit => unreachable!("handled above"),
                    Mode::SavePrompt { then } => self.key_prompt(key, then),
                    Mode::Browser(b) => self.key_browser(key, b),
                    Mode::ConfirmOverwrite { path, browser } => {
                        self.key_confirm(key, path, browser)
                    }
                }
            }
        }
    }

    /// Pasted text (bracketed paste): to the view, or typed into the file
    /// browser; a paste never answers a question.
    pub fn handle_paste(&mut self, text: &str) {
        match &self.mode {
            Mode::Edit => self.view.handle_paste(text, &mut self.shared),
            Mode::Browser(_) => {
                let first = text.lines().next().unwrap_or_default();
                for c in first.chars().filter(|c| !c.is_control()) {
                    self.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                }
            }
            Mode::SavePrompt { .. } | Mode::ConfirmOverwrite { .. } => {}
        }
    }

    /// What mdedit does when the view asks for something only the program
    /// can do.
    fn act_on(&mut self, outcome: Outcome) -> Action {
        match outcome {
            Outcome::Consumed | Outcome::Ignored => Action::Continue,
            Outcome::RequestClose if !self.view.is_dirty() => Action::Quit,
            Outcome::RequestClose => {
                self.mode = Mode::SavePrompt { then: After::Quit };
                Action::Continue
            }
            Outcome::RequestOpen if !self.view.is_dirty() => {
                self.open_picker();
                Action::Continue
            }
            Outcome::RequestOpen => {
                self.mode = Mode::SavePrompt { then: After::Open };
                Action::Continue
            }
            Outcome::RequestSaveAs => {
                self.open_save_as(After::Edit);
                Action::Continue
            }
            // mdedit edits one file: a link opens in its place (F-05).
            Outcome::OpenLink { path, heading } => {
                self.pending_link = Some((path, heading));
                if self.view.is_dirty() {
                    self.mode = Mode::SavePrompt {
                        then: After::FollowLink,
                    };
                    Action::Continue
                } else {
                    self.continue_after(After::FollowLink)
                }
            }
        }
    }

    /// `Save changes? [Y]es [N]o [C]ancel` before exiting (F-03) or opening
    /// another file (F-04).
    fn key_prompt(&mut self, key: KeyEvent, then: After) -> Action {
        match key.code {
            KeyCode::Char('y' | 'Y') => self.save(then),
            KeyCode::Char('n' | 'N') => self.continue_after(then),
            KeyCode::Char('c' | 'C') | KeyCode::Esc => Action::Continue,
            _ => {
                self.mode = Mode::SavePrompt { then };
                Action::Continue
            }
        }
    }

    /// Shows the Save As browser in the document's folder (or `cwd`), with
    /// the current file name (or `untitled.md`) filled in.
    fn open_save_as(&mut self, then: After) {
        let (dir, name) = match &self.view.path {
            Some(p) => (
                p.parent().map_or_else(|| self.cwd.clone(), PathBuf::from),
                p.file_name()
                    .map_or("untitled.md".into(), |n| n.to_string_lossy().to_string()),
            ),
            None => (self.cwd.clone(), "untitled.md".to_string()),
        };
        let dir = if dir.as_os_str().is_empty() {
            self.cwd.clone()
        } else {
            dir
        };
        self.mode = Mode::Browser(Browser::new(Purpose::SaveAs { then }, dir, name));
    }

    fn key_browser(&mut self, key: KeyEvent, mut b: Browser) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => return Action::Continue, // mode is already Edit
            KeyCode::Up => b.move_up(),
            KeyCode::Down => b.move_down(),
            KeyCode::Char('h') if ctrl => {
                b.show_all = !b.show_all;
                b.refresh();
            }
            KeyCode::Backspace => {
                b.input.pop();
                b.error = None;
                if b.purpose == Purpose::Open {
                    b.refresh();
                }
            }
            KeyCode::Char(c) if !ctrl => {
                b.input.push(c);
                b.error = None;
                if b.purpose == Purpose::Open {
                    b.refresh();
                } else {
                    b.selected = None;
                }
            }
            KeyCode::Enter => return self.browser_enter(b),
            _ => {}
        }
        self.mode = Mode::Browser(b);
        Action::Continue
    }

    /// Shows the Open file picker in the document's folder (or `cwd`).
    fn open_picker(&mut self) {
        let dir = self
            .view
            .path
            .as_deref()
            .and_then(|p| p.parent())
            .filter(|d| !d.as_os_str().is_empty())
            .map_or_else(|| self.cwd.clone(), PathBuf::from);
        self.mode = Mode::Browser(Browser::new(Purpose::Open, dir, String::new()));
    }

    /// Replaces the document with the file at `path` (F-04). If it can't be
    /// read as UTF-8 text, the picker stays open with the error.
    fn open_file(&mut self, path: PathBuf, mut b: Browser) -> Action {
        if let Err(e) = self.load(path) {
            b.error = Some(e);
            self.mode = Mode::Browser(b);
        }
        Action::Continue
    }

    /// Replaces the document with the file at `path` (cursor at the top),
    /// keeping the view's display choices. `Err` explains why it couldn't
    /// be read as UTF-8 text.
    fn load(&mut self, path: PathBuf) -> Result<(), String> {
        let mut view = EditorView::open(path)?;
        view.source_mode = self.view.source_mode;
        (view.width, view.height) = (self.view.width, self.view.height);
        self.view = view;
        self.mode = Mode::Edit;
        Ok(())
    }

    /// Moves the cursor to the heading `heading`; see
    /// [`EditorView::go_to_heading`].
    pub fn go_to_heading(&mut self, heading: &str) -> bool {
        self.view.go_to_heading(heading)
    }

    /// Enter in the browser: act on the highlighted entry, or on the input.
    fn browser_enter(&mut self, mut b: Browser) -> Action {
        // Open: a typed path wins over the highlighted entry.
        if b.purpose == Purpose::Open && is_path_like(&b.input) {
            let path = b.typed_path();
            if path.is_dir() {
                b.enter(path);
            } else if path.is_file() {
                return self.open_file(path, b);
            } else {
                b.error = Some(format!("No such file: {}", path.display()));
            }
            self.mode = Mode::Browser(b);
            return Action::Continue;
        }
        if let Some((path, is_dir)) = b.selected_path() {
            if !is_dir && b.purpose == Purpose::Open {
                return self.open_file(path, b);
            }
            if is_dir {
                b.enter(path);
            } else if let Purpose::SaveAs { .. } = b.purpose {
                b.input = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                b.selected = None;
            }
            self.mode = Mode::Browser(b);
            return Action::Continue;
        }

        let Purpose::SaveAs { then } = b.purpose else {
            self.mode = Mode::Browser(b);
            return Action::Continue;
        };
        let mut target = b.typed_path();
        if b.input.trim().is_empty() {
            b.error = Some("Type a file name".into());
        } else if target.is_dir() {
            b.input.clear();
            b.enter(target);
        } else {
            if target.extension().is_none() {
                target.set_extension("md");
            }
            match target.parent() {
                Some(dir) if !dir.is_dir() => {
                    b.error = Some(format!("Folder does not exist: {}", dir.display()));
                }
                _ if target.exists() => {
                    self.mode = Mode::ConfirmOverwrite {
                        path: target,
                        browser: b,
                    };
                    return Action::Continue;
                }
                _ => return self.write_to(target, then, b),
            }
        }
        self.mode = Mode::Browser(b);
        Action::Continue
    }

    fn key_confirm(&mut self, key: KeyEvent, path: PathBuf, browser: Browser) -> Action {
        match (key.code, browser.purpose) {
            (KeyCode::Char('y' | 'Y'), Purpose::SaveAs { then }) => {
                self.write_to(path, then, browser)
            }
            (KeyCode::Char('n' | 'N') | KeyCode::Esc, _) => {
                self.mode = Mode::Browser(browser);
                Action::Continue
            }
            _ => {
                self.mode = Mode::ConfirmOverwrite { path, browser };
                Action::Continue
            }
        }
    }

    /// Save As: writes the document to `target`, which becomes its path.
    /// On failure the browser stays open with the error.
    fn write_to(&mut self, target: PathBuf, then: After, mut b: Browser) -> Action {
        match self.view.save_as(target) {
            Ok(()) => self.continue_after(then),
            Err(e) => {
                b.error = Some(e);
                self.mode = Mode::Browser(b);
                Action::Continue
            }
        }
    }

    /// Carries on with what a save (or a "No" at the save prompt) was for.
    /// The document is only replaced once a file is actually opened, so
    /// cancelling the picker after "No" loses nothing.
    fn continue_after(&mut self, then: After) -> Action {
        match then {
            After::Edit => self.mode = Mode::Edit,
            After::FollowLink => {
                self.mode = Mode::Edit;
                if let Some((target, heading)) = self.pending_link.take() {
                    match self.load(target) {
                        Ok(()) => {
                            if let Some(heading) = heading {
                                self.view.go_to_heading(&heading);
                            }
                        }
                        Err(e) => self.view.status = e,
                    }
                }
            }
            After::Open => self.open_picker(),
            After::Quit => return Action::Quit,
        }
        Action::Continue
    }

    /// Ctrl+S or "Yes" at the save prompt (F-01): writes the document to its
    /// file, then does `then`. An untitled document goes through Save As
    /// instead. On a write error the document stays unsaved and the status
    /// line shows the error.
    fn save(&mut self, then: After) -> Action {
        if self.view.path.is_none() {
            self.open_save_as(then);
            return Action::Continue;
        }
        match self.view.save() {
            Ok(()) => self.continue_after(then),
            Err(_) => {
                self.mode = Mode::Edit;
                Action::Continue
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(key(code, KeyModifiers::NONE));
    }

    fn picker(app: &App) -> &crate::emoji::Picker {
        match &app.view.mode {
            ViewMode::EmojiPicker(p) => p,
            other => panic!("expected the emoji picker, got {other:?}"),
        }
    }

    fn open_picker_and_search(app: &mut App, query: &str) {
        app.handle_key(key(KeyCode::Char('e'), KeyModifiers::CONTROL));
        for c in query.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn ctrl_e_opens_the_emoji_picker_without_changing_text() {
        let mut app = App::new("hi", None);
        app.handle_key(key(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert_eq!(picker(&app).query, "");
        assert!(!app.view.editor.dirty);
    }

    #[test]
    fn typing_in_the_picker_searches() {
        let mut app = App::new("hi", None);
        open_picker_and_search(&mut app, "cat");
        assert_eq!(picker(&app).query, "cat");
        assert_eq!(picker(&app).current().unwrap().as_str(), "🐱");
        press(&mut app, KeyCode::Backspace);
        assert_eq!(picker(&app).query, "ca");
        assert_eq!(
            app.view.editor.lines,
            ["hi"],
            "typing goes to the search, not the text"
        );
    }

    #[test]
    fn enter_inserts_at_cursor_and_closes() {
        let mut app = App::new("ab", None);
        app.view.editor.col = 1;
        open_picker_and_search(&mut app, "grinning");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.view.editor.lines, ["a😀b"]);
        assert_eq!(app.view.editor.col, 2, "cursor after the emoji");
        assert!(app.view.editor.dirty);
    }

    #[test]
    fn inserted_emoji_is_remembered_and_shown_first_next_time() {
        let dir = std::env::temp_dir().join("mdedit-app-tests").join("recent");
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new("", None);
        app.shared.recent.path = Some(dir.join("recent_emoji"));

        open_picker_and_search(&mut app, "cat");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.shared.recent.items[0].as_str(), "🐱");
        assert_eq!(
            std::fs::read_to_string(dir.join("recent_emoji")).unwrap(),
            "🐱\n"
        );

        open_picker_and_search(&mut app, "");
        assert_eq!(picker(&app).current().unwrap().as_str(), "🐱");
    }

    #[test]
    fn failing_to_save_recent_still_inserts() {
        let mut app = App::new("", None);
        // A path under a file can't be created.
        let blocker = std::env::temp_dir().join("mdedit-app-tests-blocker");
        std::fs::write(&blocker, "x").unwrap();
        app.shared.recent.path = Some(blocker.join("sub").join("recent_emoji"));
        open_picker_and_search(&mut app, "cat");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.view.editor.lines, ["🐱"]);
        assert!(
            app.view.status.contains("recent"),
            "status: {}",
            app.view.status
        );
    }

    #[test]
    fn arrows_choose_which_result_is_inserted() {
        let mut app = App::new("", None);
        open_picker_and_search(&mut app, "cat");
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.view.editor.lines, ["🐈"]);
    }

    #[test]
    fn esc_closes_the_picker_without_inserting() {
        let mut app = App::new("ab", None);
        open_picker_and_search(&mut app, "cat");
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.view.editor.lines, ["ab"]);
        assert!(!app.view.editor.dirty);
    }

    #[test]
    fn ctrl_e_again_closes_the_picker_like_esc() {
        let mut app = App::new("ab", None);
        open_picker_and_search(&mut app, "cat");
        app.handle_key(key(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.view.editor.lines, ["ab"]);
        assert!(!app.view.editor.dirty);
    }

    #[test]
    fn enter_with_no_results_keeps_the_picker_open() {
        let mut app = App::new("ab", None);
        open_picker_and_search(&mut app, "zzqxj");
        press(&mut app, KeyCode::Enter);
        assert_eq!(picker(&app).query, "zzqxj");
        assert_eq!(app.view.editor.lines, ["ab"]);
    }

    fn ctrl_k(app: &mut App) {
        app.handle_key(key(KeyCode::Char('k'), KeyModifiers::CONTROL));
    }

    const FOLDED: &str = "> [!tip]- T\n> one\n> two\nafter";

    #[test]
    fn ctrl_k_toggles_the_callout_under_the_cursor() {
        let mut app = App::new(FOLDED, None);
        ctrl_k(&mut app);
        assert_eq!(app.view.folds.get(&0), Some(&false), "expanded");
        ctrl_k(&mut app);
        assert_eq!(app.view.folds.get(&0), Some(&true), "collapsed again");
    }

    #[test]
    fn collapsing_from_the_body_moves_the_cursor_to_the_header() {
        let mut app = App::new("> [!tip]+ T\n> one\n> two", None);
        app.view.editor.row = 2;
        ctrl_k(&mut app);
        assert_eq!(app.view.folds.get(&0), Some(&true));
        assert_eq!(app.view.editor.row, 0);
    }

    #[test]
    fn ctrl_k_with_nothing_to_fold_says_so() {
        let mut app = App::new("> [!tip] T\n> one\nplain", None);
        ctrl_k(&mut app);
        assert!(app.view.folds.is_empty());
        assert!(
            app.view.status.contains("fold"),
            "status: {}",
            app.view.status
        );
    }

    #[test]
    fn ctrl_k_folds_the_innermost_section_or_list_item() {
        let mut app = App::new("# A\n- item\n  - sub\ntext", None);
        app.view.editor.row = 1;
        ctrl_k(&mut app);
        assert_eq!(app.view.folds.get(&1), Some(&true), "the list item");
        app.view.editor.row = 3;
        ctrl_k(&mut app);
        assert_eq!(app.view.folds.get(&0), Some(&true), "the section");
        assert_eq!(app.view.editor.row, 0, "cursor moves to the heading");
        assert!(
            app.view.status.contains("3 lines"),
            "status: {}",
            app.view.status
        );
    }

    #[test]
    fn up_and_down_skip_a_collapsed_body() {
        let mut app = App::new(FOLDED, None);
        press(&mut app, KeyCode::Down);
        assert_eq!(app.view.editor.row, 3, "skipped the hidden lines");
        press(&mut app, KeyCode::Up);
        assert_eq!(app.view.editor.row, 0);
    }

    #[test]
    fn moving_into_a_hidden_line_opens_the_callout() {
        let mut app = App::new(FOLDED, None);
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Right); // onto hidden line 1
        assert_eq!(app.view.editor.row, 1);
        assert_eq!(app.view.folds.get(&0), Some(&false));
    }

    #[test]
    fn adding_or_removing_lines_resets_folds_to_their_defaults() {
        let mut app = App::new(FOLDED, None);
        ctrl_k(&mut app);
        app.view.editor.row = 3;
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Enter);
        assert!(app.view.folds.is_empty());
    }

    fn lines(n: usize) -> String {
        (1..=n).map(|i| format!("line {i}\n")).collect()
    }

    #[test]
    fn page_down_and_up_move_a_screen_and_scroll_with_it() {
        let mut app = App::new(&lines(30), None);
        app.view.width = 40;
        app.view.height = 10;
        press(&mut app, KeyCode::PageDown);
        assert_eq!(
            app.view.editor.row, 9,
            "one screen minus one row, for context"
        );
        assert_eq!(app.view.scroll, 9);
        press(&mut app, KeyCode::PageDown);
        assert_eq!(app.view.editor.row, 18);
        press(&mut app, KeyCode::PageUp);
        assert_eq!((app.view.editor.row, app.view.scroll), (9, 9));
        press(&mut app, KeyCode::PageUp);
        assert_eq!((app.view.editor.row, app.view.scroll), (0, 0));
    }

    #[test]
    fn page_keys_stop_at_the_ends() {
        let mut app = App::new(&lines(5), None);
        app.view.width = 40;
        app.view.height = 10;
        press(&mut app, KeyCode::PageDown);
        assert_eq!(app.view.editor.row, 4);
        press(&mut app, KeyCode::PageUp);
        assert_eq!(app.view.editor.row, 0);
    }

    #[test]
    fn page_keys_keep_the_column() {
        let mut app = App::new(&lines(30), None);
        app.view.width = 40;
        app.view.height = 10;
        app.view.editor.col = 3;
        press(&mut app, KeyCode::PageDown);
        assert_eq!(app.view.editor.col, 3);
    }

    const HAYSTACK: &str = "# Notes\nalpha beta\nGamma delta\nbeta again\nlast beta";

    fn search_for(app: &mut App, query: &str) {
        app.handle_key(key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        for c in query.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    fn pos(app: &App) -> (usize, usize) {
        (app.view.editor.row, app.view.editor.col)
    }

    #[test]
    fn typing_a_search_jumps_to_the_first_match() {
        let mut app = App::new(HAYSTACK, None);
        search_for(&mut app, "bet");
        assert!(matches!(app.view.mode, ViewMode::Search(_)));
        assert_eq!(pos(&app), (1, 6));
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(pos(&app), (1, 6));
        assert!(!app.view.editor.dirty, "typing goes to the prompt");
    }

    #[test]
    fn arrows_and_f3_move_between_matches_in_the_prompt() {
        let mut app = App::new(HAYSTACK, None);
        search_for(&mut app, "beta");
        press(&mut app, KeyCode::Down);
        assert_eq!(pos(&app), (3, 0));
        press(&mut app, KeyCode::F(3));
        assert_eq!(pos(&app), (4, 5));
        press(&mut app, KeyCode::Up);
        assert_eq!(pos(&app), (3, 0));
    }

    #[test]
    fn enter_keeps_the_position_and_esc_goes_back() {
        let mut app = App::new(HAYSTACK, None);
        search_for(&mut app, "delta");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(pos(&app), (2, 6));

        let mut app = App::new(HAYSTACK, None);
        search_for(&mut app, "delta");
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(pos(&app), (0, 0));
    }

    #[test]
    fn f3_and_shift_f3_repeat_the_last_search_while_editing() {
        let mut app = App::new(HAYSTACK, None);
        search_for(&mut app, "beta");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::F(3));
        assert_eq!(pos(&app), (3, 0));
        app.handle_key(key(KeyCode::F(3), KeyModifiers::SHIFT));
        assert_eq!(pos(&app), (1, 6));
    }

    #[test]
    fn no_match_says_so_and_keeps_the_cursor() {
        let mut app = App::new(HAYSTACK, None);
        search_for(&mut app, "zzz");
        assert_eq!(pos(&app), (0, 0));
        assert!(
            app.view.status.contains("No match"),
            "status: {}",
            app.view.status
        );
        let mut app = App::new(HAYSTACK, None);
        press(&mut app, KeyCode::F(3));
        assert!(
            app.view.status.contains("Ctrl+F"),
            "status: {}",
            app.view.status
        );
    }

    #[test]
    fn search_opens_a_folded_callout_it_lands_in() {
        let mut app = App::new("> [!tip]- T\n> secret\nafter", None);
        search_for(&mut app, "secret");
        assert_eq!(pos(&app), (1, 2));
        assert_eq!(app.view.folds.get(&0), Some(&false));
    }

    #[test]
    fn ctrl_v_toggles_source_mode() {
        let mut app = App::new("# T\ntext", None);
        app.handle_key(key(KeyCode::Char('v'), KeyModifiers::CONTROL));
        assert!(app.view.source_mode);
        assert!(
            app.view.status.contains("Source"),
            "status: {}",
            app.view.status
        );
        app.handle_key(key(KeyCode::Char('v'), KeyModifiers::CONTROL));
        assert!(!app.view.source_mode);
        assert!(!app.view.editor.dirty, "no text is typed");
    }

    #[test]
    fn source_mode_shows_folded_lines_to_up_and_down() {
        let mut app = App::new(FOLDED, None);
        app.view.source_mode = true;
        press(&mut app, KeyCode::Down);
        assert_eq!(app.view.editor.row, 1, "nothing is hidden in source mode");
    }

    fn replace_prompt(app: &mut App, find: &str, with: &str) {
        app.handle_key(key(KeyCode::Char('h'), KeyModifiers::CONTROL));
        for c in find.chars() {
            press(app, KeyCode::Char(c));
        }
        press(app, KeyCode::Tab);
        for c in with.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    const CATS: &str = "one cat, two cat\nred cat";

    #[test]
    fn replace_prompt_finds_as_you_type() {
        let mut app = App::new(CATS, None);
        replace_prompt(&mut app, "cat", "dog");
        assert!(matches!(app.view.mode, ViewMode::Replace(_)));
        assert_eq!(pos(&app), (0, 4));
        assert_eq!(
            app.view.editor.lines[0], "one cat, two cat",
            "nothing replaced yet"
        );
    }

    #[test]
    fn enter_replaces_the_match_and_goes_to_the_next() {
        let mut app = App::new(CATS, None);
        replace_prompt(&mut app, "cat", "dog");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.view.editor.lines[0], "one dog, two cat");
        assert_eq!(pos(&app), (0, 13));
        assert!(app.view.editor.dirty);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.view.editor.lines, ["one dog, two dog", "red dog"]);
        press(&mut app, KeyCode::Enter);
        assert!(
            app.view.status.contains("No match"),
            "status: {}",
            app.view.status
        );
    }

    #[test]
    fn ctrl_a_replaces_all_and_counts() {
        let mut app = App::new(CATS, None);
        replace_prompt(&mut app, "cat", "dog");
        app.handle_key(key(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert_eq!(app.view.editor.lines, ["one dog, two dog", "red dog"]);
        assert!(
            app.view.status.contains("Replaced 3"),
            "status: {}",
            app.view.status
        );
    }

    #[test]
    fn down_skips_a_match_without_replacing_and_esc_closes() {
        let mut app = App::new(CATS, None);
        replace_prompt(&mut app, "cat", "dog");
        press(&mut app, KeyCode::Down);
        assert_eq!(pos(&app), (0, 13));
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.view.editor.lines[0], "one cat, two cat");
        assert!(!app.view.editor.dirty);
    }

    fn ctrl_key(app: &mut App, c: char) {
        app.handle_key(key(KeyCode::Char(c), KeyModifiers::CONTROL));
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn ctrl_z_undoes_typing_a_word_at_a_time_and_ctrl_y_redoes() {
        let mut app = App::new("one", None);
        press(&mut app, KeyCode::End);
        type_text(&mut app, " two three");
        ctrl_key(&mut app, 'z');
        assert_eq!(app.view.editor.lines, ["one two"]);
        assert_eq!(pos(&app), (0, 7));
        ctrl_key(&mut app, 'z');
        assert_eq!(app.view.editor.lines, ["one"]);
        assert!(!app.view.editor.dirty, "back to the text as it was opened");
        ctrl_key(&mut app, 'y');
        assert_eq!(app.view.editor.lines, ["one two"]);
        assert!(app.view.editor.dirty);
        app.handle_key(key(
            KeyCode::Char('Z'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(
            app.view.editor.lines,
            ["one two three"],
            "Ctrl+Shift+Z redoes too"
        );
    }

    #[test]
    fn undo_goes_back_as_many_steps_as_configured() {
        let mut app = App::new("", None);
        type_text(&mut app, "a b c d e f g");
        for _ in 0..10 {
            ctrl_key(&mut app, 'z');
        }
        assert_eq!(app.view.editor.lines, ["a b"], "5 words undone, by default");
        assert!(
            app.view.status.contains("Nothing to undo"),
            "{}",
            app.view.status
        );

        let mut app = App::new("", None);
        app.shared.config.undo_steps = 2;
        type_text(&mut app, "a b c");
        for _ in 0..3 {
            ctrl_key(&mut app, 'z');
        }
        assert_eq!(app.view.editor.lines, ["a"]);
    }

    #[test]
    fn enter_and_list_continuation_undo_in_one_step() {
        let mut app = App::new("- a", None);
        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.view.editor.lines, ["- a", "- "]);
        ctrl_key(&mut app, 'z');
        assert_eq!(app.view.editor.lines, ["- a"]);
        assert_eq!(pos(&app), (0, 3));
    }

    #[test]
    fn replace_all_and_paste_undo_in_one_step() {
        let mut app = App::new(CATS, None);
        replace_prompt(&mut app, "cat", "dog");
        ctrl_key(&mut app, 'a');
        app.handle_paste("x\ny");
        ctrl_key(&mut app, 'z');
        assert_eq!(app.view.editor.lines, ["one dog, two dog", "red dog"]);
        ctrl_key(&mut app, 'z');
        assert_eq!(app.view.editor.lines, CATS.lines().collect::<Vec<_>>());
    }

    #[test]
    fn nothing_to_undo_says_so() {
        let mut app = App::new("a", None);
        ctrl_key(&mut app, 'z');
        assert!(
            app.view.status.contains("Nothing to undo"),
            "{}",
            app.view.status
        );
        ctrl_key(&mut app, 'y');
        assert!(
            app.view.status.contains("Nothing to redo"),
            "{}",
            app.view.status
        );
    }

    fn shift(app: &mut App, code: KeyCode) {
        app.handle_key(key(code, KeyModifiers::SHIFT));
    }

    #[test]
    fn shift_arrows_select_and_typing_replaces_the_selection() {
        let mut app = App::new("one two three", None);
        app.view.editor.col = 4;
        for _ in 0..3 {
            shift(&mut app, KeyCode::Right);
        }
        assert_eq!(app.view.editor.selected_text().as_deref(), Some("two"));
        type_text(&mut app, "2");
        assert_eq!(app.view.editor.lines, ["one 2 three"]);
        assert_eq!(app.view.editor.selection(), None);
        ctrl_key(&mut app, 'z');
        assert_eq!(app.view.editor.lines, ["one two three"], "one undo step");
    }

    #[test]
    fn shift_down_and_backspace_delete_across_lines() {
        let mut app = App::new("ab\ncd\nef", None);
        app.view.editor.col = 1;
        shift(&mut app, KeyCode::Down);
        shift(&mut app, KeyCode::End);
        assert_eq!(app.view.editor.selected_text().as_deref(), Some("b\ncd"));
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.view.editor.lines, ["a", "ef"]);
    }

    #[test]
    fn a_plain_move_or_esc_ends_the_selection() {
        let mut app = App::new("abc", None);
        shift(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.view.editor.selection(), None);
        shift(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.view.editor.selection(), None);
        assert_eq!(app.view.editor.lines, ["abc"]);
    }

    #[test]
    fn ctrl_a_selects_everything() {
        let mut app = App::new("a\nb", None);
        ctrl_key(&mut app, 'a');
        press(&mut app, KeyCode::Delete);
        assert_eq!(app.view.editor.lines, [""]);
    }

    #[test]
    fn tab_indents_every_selected_line() {
        let mut app = App::new("- a\n- b\n- c", None);
        app.view.editor.row = 1;
        shift(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.view.editor.lines, ["- a", "  - b", "  - c"]);
        app.handle_key(key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.view.editor.lines, ["- a", "- b", "- c"]);
    }

    #[test]
    fn wrap_characters_wrap_the_selection() {
        let cases = [
            ("*", "a *word* here"),
            ("**", "a **word** here"),
            ("_", "a _word_ here"),
            ("~~", "a ~~word~~ here"),
            ("==", "a ==word== here"),
            ("`", "a `word` here"),
            ("(", "a (word) here"),
            ("[[", "a [[word]] here"),
            ("\"", "a \"word\" here"),
        ];
        for (typed, want) in cases {
            let mut app = App::new("a word here", None);
            app.view.editor.col = 2;
            for _ in 0..4 {
                shift(&mut app, KeyCode::Right);
            }
            type_text(&mut app, typed);
            assert_eq!(app.view.editor.lines, [want], "typing {typed}");
            assert_eq!(app.view.editor.selected_text().as_deref(), Some("word"));
        }
    }

    #[test]
    fn pasting_a_url_over_a_selection_makes_a_link() {
        let mut app = App::new("see the docs", None);
        app.view.editor.col = 8;
        shift(&mut app, KeyCode::End);
        app.handle_paste("https://example.com/a?b=1");
        assert_eq!(
            app.view.editor.lines,
            ["see the [docs](https://example.com/a?b=1)"]
        );
        assert_eq!(pos(&app), (0, 41));
    }

    #[test]
    fn pasting_other_text_replaces_the_selection() {
        let mut app = App::new("see the docs", None);
        app.view.editor.col = 8;
        shift(&mut app, KeyCode::End);
        app.handle_paste("not a url");
        assert_eq!(app.view.editor.lines, ["see the not a url"]);
        let mut app = App::new("x", None);
        app.handle_paste("https://example.com");
        assert_eq!(
            app.view.editor.lines,
            ["https://example.comx"],
            "no selection: plain paste"
        );
    }

    #[test]
    fn auto_pair_can_be_turned_off() {
        let mut app = App::new("", None);
        type_text(&mut app, "(");
        assert_eq!(app.view.editor.lines, ["()"]);
        let mut app = App::new("", None);
        app.shared.config.auto_pair = false;
        type_text(&mut app, "(");
        assert_eq!(app.view.editor.lines, ["("]);
    }

    #[test]
    fn paste_inserts_text_exactly_without_list_continuation() {
        let mut app = App::new("start  end", None);
        app.view.editor.col = 6;
        app.handle_paste("- a\r\n- b\n\tc\n1. x");
        assert_eq!(
            app.view.editor.lines,
            ["start - a", "- b", "\tc", "1. x end"],
            "no `- - b`, no indent from the tab, no renumbering"
        );
        assert_eq!(pos(&app), (3, 4));
        assert!(app.view.editor.dirty);
    }

    #[test]
    fn paste_into_search_uses_the_first_line() {
        let mut app = App::new(CATS, None);
        app.handle_key(key(KeyCode::Char('f'), KeyModifiers::CONTROL));
        app.handle_paste("red\nignored");
        assert!(matches!(&app.view.mode, ViewMode::Search(s) if s.query == "red"));
        assert_eq!(pos(&app), (1, 0));
    }

    #[test]
    fn paste_never_answers_a_yes_no_prompt() {
        let mut app = App::new("x", None);
        press(&mut app, KeyCode::Char('!'));
        app.handle_key(key(KeyCode::Char('x'), KeyModifiers::CONTROL));
        app.handle_paste("n");
        assert!(matches!(app.mode, Mode::SavePrompt { .. }));
    }

    #[test]
    fn up_and_down_move_by_screen_row_in_wrapped_line() {
        // At width 10 the line shows as "aaaa bbbb" / "cccc".
        let mut app = App::new("aaaa bbbb cccc\nnext", None);
        app.view.width = 10;
        app.view.editor.col = 12; // row 1, x 2
        press(&mut app, KeyCode::Up);
        assert_eq!((app.view.editor.row, app.view.editor.col), (0, 2));
        press(&mut app, KeyCode::Down);
        assert_eq!((app.view.editor.row, app.view.editor.col), (0, 12));
        press(&mut app, KeyCode::Down);
        assert_eq!((app.view.editor.row, app.view.editor.col), (1, 2));
        press(&mut app, KeyCode::Up);
        assert_eq!((app.view.editor.row, app.view.editor.col), (0, 12));
    }

    #[test]
    fn up_into_wrapped_line_lands_on_its_last_row() {
        let mut app = App::new("aaaa bbbb cccc\nxy", None);
        app.view.width = 10;
        app.view.editor.row = 1;
        app.view.editor.col = 1;
        press(&mut app, KeyCode::Up);
        assert_eq!((app.view.editor.row, app.view.editor.col), (0, 11));
    }

    #[test]
    fn vertical_move_without_width_is_line_based() {
        let mut app = App::new("aaaa bbbb cccc\nnext", None);
        app.view.editor.col = 12;
        press(&mut app, KeyCode::Down);
        assert_eq!((app.view.editor.row, app.view.editor.col), (1, 4));
    }
}
