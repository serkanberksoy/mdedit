# Embedding mdedit

mdedit is a program (`mdedit note.md`) and a library. Another terminal
program, such as a vault app with tabs and a file list, can use the library
to embed the editor. mdedit itself contains no host code: no tabs, no file
list, no vault. It offers the pieces below, and its own `mdedit` binary is
built on them (`src/app.rs`, `src/main.rs`).

The API is covered by `tests/embedding.rs` and by the example in
`src/lib.rs`, which is compiled and run as a doctest.

## Adding the dependency

```toml
[dependencies]
mdedit = { path = "../mdedit" }   # or a git URL / tag
ratatui = "0.30"                  # must be the same major version as mdedit's
```

`Frame`, `Buffer` and `Rect` cross the boundary, so the host and mdedit
must use the same ratatui version.

## The pieces

| Type | One per | What it holds |
|------|---------|---------------|
| `shared::Shared` | program | `config` (settings), `caps` (terminal colors and Unicode), `picker` + `image_cache` (images), `recent` (emoji), `resolver` (link targets) |
| `view::EditorView` | open document (tab, pane) | text, cursor, selection, undo history, folds, scroll, source mode, search / replace / emoji prompts, status message |
| `ui::EditorWidget` | frame | draws a view into any `Rect` |
| `resolver::Resolver` | program | trait: finds link, embed and image targets |

## Setup, once per program

```rust
let mut shared = mdedit::shared::Shared::new();
shared.caps = mdedit::terminal::Capabilities::from_env();
shared.config = /* your settings, or mdedit::config::Config::load(path).0 */;
// After raw mode is on (it asks the terminal):
shared.picker = mdedit::images::picker_for(shared.config.images);
shared.resolver = Box::new(MyVaultResolver { /* … */ });
```

Also turn on bracketed paste (`EnableBracketedPaste`) and, where supported,
the kitty keyboard protocol (`DISAMBIGUATE_ESCAPE_CODES`), as
`src/main.rs` does. Pastes go to `view.handle_paste(&text, &mut shared)`.

## Documents

```rust
let view = EditorView::open(path)?;         // a file (Err: not readable / not UTF-8)
let view = EditorView::new(text, None);     // untitled
view.is_dirty(); view.title(); view.path;   // for tab labels
view.save()?; view.save_as(path)?;          // safe writes (F-07); status says what happened
view.go_to_heading("My Title");             // after opening a link with a heading
view.status_line("");                       // for the host's own status bar
```

## Keys and outcomes

The host gets each key first (tab switching, its own shortcuts); the rest
go to the active view:

```rust
match view.handle_key(key, &mut shared) {
    Outcome::Consumed => {}
    Outcome::Ignored => { /* the host's keymap */ }
    Outcome::OpenLink { path, heading } => { /* open/focus a tab, then go_to_heading */ }
    Outcome::RequestOpen => { /* focus the file list */ }
    Outcome::RequestSaveAs => { /* ask for a name, then view.save_as(path) */ }
    Outcome::RequestClose => { /* if view.is_dirty(), ask to save; then close the tab */ }
}
```

The view handles everything else itself: editing, selection, undo, search
and replace, folding, the emoji picker, Ctrl+S for a document that has a
file, and links to headings in the same note.

## Drawing

```rust
terminal.draw(|frame| {
    // … the host's file list, tab bar …
    frame.render_stateful_widget(
        EditorWidget::new(&mut shared).status_bar(true), // false: use view.status_line() yourself
        editor_area,
        &mut view,
    );
    // Once per frame, after everything is drawn:
    mdedit::ui::apply_terminal_workarounds(frame.buffer_mut(), &shared.caps, &view.image_areas);
    if let Some(cursor) = view.cursor {
        frame.set_cursor_position(cursor);
    }
})?;
```

- The view's prompts (search, replace) use the status row, or the area's
  last row without a status bar; the emoji picker is centered in the area.
- Double-size headings only work in an area as wide as the screen (the
  terminal sizes whole rows), so they're off in a narrower area.
- Only draw the visible tabs: the caches (block structure, highlighting,
  embeds, images) are shared by content, so switching tabs is cheap.

## Resolving links in a vault

```rust
struct MyVaultResolver { index: /* name → path */ }

impl mdedit::resolver::Resolver for MyVaultResolver {
    fn resolve(&self, from: Option<&Path>, target: &str) -> Result<PathBuf, String> {
        // `[[Note]]` by name anywhere in the vault; Err explains, for the status line.
    }
    // Optional: `exists` (unresolved-link styling) and `load` (embed
    // content from your own cache); both have defaults.
}
```

The resolver is used by Ctrl+Enter (`Outcome::OpenLink`), note embeds
(`![[Note]]`, `![[Note#Heading]]`) and image embeds (`![[photo.png]]`).
mdedit's own `RelativeResolver` looks next to the current note.

## What stays in the mdedit program

The Open / Save As file browser, the "Save changes?" prompt, following a
link in place, exiting, the status bar key hints (`ui::KEY_HINTS`) and
loading `config.toml` are in `src/app.rs` and `src/main.rs`, not in the
library API. A host brings its own.
