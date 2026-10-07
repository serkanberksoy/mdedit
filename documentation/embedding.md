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
| `shared::Shared` | program | `config` (settings), `caps` (terminal colors and Unicode), `picker` + `image_cache` (images), `recent` (emoji), `resolver` (link targets), `processor` (the host's code blocks), `clipboard` (where Ctrl+V reads from) |
| `view::EditorView` | open document (tab, pane) | text, cursor, selection, undo history, folds, scroll, source mode, search / replace / emoji prompts, status message |
| `ui::EditorWidget` | frame | draws a view into any `Rect` |
| `resolver::Resolver` | program | trait: finds link, embed and image targets |
| `processor::CodeBlockProcessor` | program | trait, optional: renders fenced blocks of the host's languages |

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
    Outcome::Action(action) => { /* a code block result row's action (view mode, clicks) */ }
    Outcome::OpenUrl(url) => { /* a web link: open it in a browser (or say why not) */ }
    Outcome::MissingLink { target, heading } => { /* a link to a missing note: offer to create it */ }
    _ => {}                          // `Outcome` is non-exhaustive: new outcomes may come
}
```

The view handles everything else itself: editing, selection, undo, search
and replace, folding, the emoji picker, Ctrl+S for a document that has a
file, and links to headings in the same note.

## Mouse clicks

```rust
// A left click at (column, row): follows a link, an embed, an image or a
// code block result row under it (and moves the row cursor in view mode);
// anywhere else in the text it places the cursor.
match view.click(Position::new(x, y), &mut shared) {
    Outcome::Ignored => { /* outside the text: the host's */ }
    other => { /* handle like a key's outcome (Consumed: the cursor moved) */ }
}
```

The view remembers where it drew each row (`screen_rows`, `text_area`),
so call it after a frame. On the line being edited (raw text) and in
source mode the cursor goes to the clicked column; on a rendered line to
where the clicked text is in the source (hidden markup and glyphs lined
up). A selection ends.

A block your code block processor renders keeps the cursor out: a click
on a result row with an action or a link acts, anywhere else in the block
does nothing. Its source shows when the cursor reaches it by keys, or by
its `</>` button: send mouse moves to `view.hover(Some(Position::new(x,
y)))` (`None` when the mouse leaves); the block under the mouse is framed
in the accent color with the button on its top row, and a click on the
button puts the cursor in the source. `hover` returns true when the
hovered block changed, so draw again only then.

The mouse wheel: `view.scroll_rows(3)` (or `-3`) scrolls the live preview
by screen rows without moving the cursor, through a tall rendered block
too (a query's long result) instead of into its source; the next key or
paste goes back to the cursor. In view mode, send Up / Down instead (its
row cursor moves).

## View mode

`view.reading` is view mode (V-12, Alt+V cycles the
modes, `view.enter_reading()` turns it on): read-only, every line
rendered, a row cursor over rendered rows, Tab / Enter follow links. A
code block processor gives its rows actions with `render_rows`; Enter or a
click on such a row returns `Outcome::Action(action)`. `EditorView::read_action(&shared)` tells which action the cursor's row has in view mode without acting (for a host command that works on "the result the cursor is on").

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
- A theme: `shared.palette` swaps the colors the editor draws with, like a
  terminal color scheme: each of the 16 named colors (`palette.set(Color::
  LightBlue, link_color)`), the default text (`palette.text`) and the
  background (`palette.background`, e.g. a light page on a dark terminal).
  The default changes nothing; `apply_terminal_workarounds` fits the
  result to the terminal's colors.
- Your own inline syntax: `mdedit::markdown::set_verbatim(&[("<%", "%>")])`
  shows text from each opening marker to its closing one as written (the
  markers too), in the code color, without reading Markdown in it (a
  template language's tags). Set it once on the thread that draws; `&[]`
  turns it off.
- Your own text beside links and in place of spans:
  `markdown::set_link_badge(Some(Rc::new(|target| …)))` shows a short text
  dimmed after a wiki link (how many notes link to it), and
  `markdown::set_rendered(vec![("[@".into(), "]".into(), Rc::new(|key| …))])`
  shows what the function gives for the text between the markers (a
  citation as `Doe 2020`), in a link's color; `None` from either leaves
  things as they are. Both are asked while drawing: keep them quick.
- Lines of your own to hide: `markdown::set_hidden_lines(Some(Rc::new(|line|
  line.starts_with("<!-- TBLFM:"))))` hides such lines in the live preview
  and view mode; the cursor still moves onto one, and it shows as written
  while the cursor is on it.
- Table cells of your own: `markdown::set_table_cells(Some(Rc::new(|lines|
  …)))` gets a table's lines (its second the separator) and may give lines
  to show instead (computed cells: `=SUM(B2:B4)` as its result); the table
  shows as written while the cursor is in it.

## Resolving links in a vault

```rust
struct MyVaultResolver { index: /* name → path */ }

impl mdedit::resolver::Resolver for MyVaultResolver {
    fn resolve(&self, from: Option<&Path>, target: &str) -> Result<PathBuf, String> {
        // `[[Note]]` by name anywhere in the vault; Err explains, for the status line.
    }
    // Optional: `exists` (links to missing notes are dimmed; it's asked
    // for every wiki link drawn, so make it quick), `load` (embed
    // content from your own cache) and `embed` (what `![[file#part]]`
    // shows: other kinds of files too, e.g. a code block your processor
    // renders); all have defaults.
}
```

A host can fill a margin left of the text (Git change marks, line authors):
`view.margin_width = 2; view.margin.insert(line, Line::from("+"))`. The
text, the cursor and clicks move right by its width.

A document without a file (a host's help page, a diff) can have a name:
`view.name = Some("Help".into())`; `title()` and the status line use it.

The resolver is used by Ctrl+Enter (`Outcome::OpenLink`), note embeds
(`![[Note]]`, `![[Note#Heading]]`) and image embeds (`![[photo.png]]`).
mdedit's own `RelativeResolver` looks next to the current note.

## Rendering a host's code blocks

```rust
struct Queries { /* … */ }

impl mdedit::processor::CodeBlockProcessor for Queries {
    fn handles(&self, lang: &str) -> bool { lang == "dataview" }
    fn render(&self, lang: &str, source: &[String], from: Option<&Path>, width: usize)
        -> Vec<Line<'static>> { /* run the query; cache it, this runs every frame */ }
}

shared.processor = Some(Box::new(Queries { /* … */ }));
```

A ```` ```dataview ```` block then shows what `render` returns, in the
block's frame; with the cursor or a selection in the block, its source is
shown to edit it. Blocks in other languages are shown as code.

## What stays in the mdedit program

The Open / Save As file browser, the "Save changes?" prompt, following a
link in place, exiting, the status bar key hints (`ui::KEY_HINTS`) and
loading `config.toml` are in `src/app.rs` and `src/main.rs`, not in the
library API. A host brings its own.
