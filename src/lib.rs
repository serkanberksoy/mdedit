//! mdedit: terminal Markdown editor with Obsidian-style live preview.
//!
//! The `mdedit` program edits one file. The library also lets another
//! program (e.g. one with tabs and a file list) embed the editor; see
//! `documentation/embedding.md`. In short:
//!
//! - [`shared::Shared`]: one per program: settings, terminal
//!   capabilities, image drawing, recent emoji, and the
//!   [`resolver::Resolver`] that finds link targets.
//! - [`view::EditorView`]: one per open document (a tab or pane).
//! - [`ui::EditorWidget`]: draws a view into any area.
//! - [`view::EditorView::handle_key`] edits the document and returns an
//!   [`view::Outcome`] for what only the program can do (open a link in a
//!   tab, save under a new name, close).
//!
//! ```
//! use mdedit::shared::Shared;
//! use mdedit::ui::{EditorWidget, apply_terminal_workarounds};
//! use mdedit::view::{EditorView, Outcome};
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use ratatui::{Terminal, backend::TestBackend};
//!
//! let mut shared = Shared::new();
//! let mut tabs = vec![
//!     EditorView::new("# One\ntext", None),
//!     EditorView::new("- [ ] two", None),
//! ];
//! let active = 0;
//!
//! // Keys go to the active tab; what it doesn't use is the program's.
//! let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
//! match tabs[active].handle_key(key, &mut shared) {
//!     Outcome::Ignored => { /* e.g. the program's quick switcher */ }
//!     Outcome::OpenLink { .. } => { /* open the path in a tab, go to the heading */ }
//!     Outcome::RequestClose => { /* ask to save if tabs[active].is_dirty() */ }
//!     _ => {}
//! }
//!
//! // Drawing: a file list on the left, the active tab on the right.
//! let mut terminal = Terminal::new(TestBackend::new(60, 10)).unwrap();
//! terminal.draw(|frame| {
//!     let editor = Rect::new(20, 0, 40, 10);
//!     frame.render_stateful_widget(EditorWidget::new(&mut shared), editor, &mut tabs[active]);
//!     apply_terminal_workarounds(frame.buffer_mut(), &shared.caps, &tabs[active].image_areas);
//!     if let Some(cursor) = tabs[active].cursor {
//!         frame.set_cursor_position(cursor);
//!     }
//! }).unwrap();
//! ```

pub mod app;
pub mod autopair;
pub mod blocks;
pub mod cli;
pub mod config;
pub mod editor;
pub mod embed;
pub mod emoji;
pub mod files;
pub mod highlight;
pub mod history;
pub mod images;
pub mod links;
pub mod markdown;
pub mod resolver;
pub mod search;
pub mod selection;
pub mod shared;
pub mod terminal;
pub mod ui;
pub mod view;
pub mod wrap;
