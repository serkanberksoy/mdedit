# Version history

Current version: **2.0.0**

## Versioning rules

mdedit uses [Semantic Versioning](https://semver.org). Before 1.0:

| Change | Bump | Example |
|--------|------|---------|
| A feature is implemented (a row in `requirements/obsidian_features.md` moves to ✅ or 🟡, or an F-/R- requirement is met) | **MINOR** | 0.2.0 → 0.3.0 |
| Bug fix, refactor, docs or tests only | **PATCH** | 0.3.0 → 0.3.1 |
| Milestone M1 complete | **1.0.0** | |
| An incompatible change to the library API (`documentation/embedding.md`) | **MAJOR** | 1.8.1 → 2.0.0 |

On every bump:
1. Update `version` in `Cargo.toml`.
2. Add a new entry at the **top** of this file (newest first), and update
   "Current version".
3. Update `README.md` (the version line, plus the feature status if it
   changed).
4. Update `example_mds/feature_showcase.md`: show the new feature and set
   its frontmatter `version:`.

`cargo test` fails (`tests/project.rs`) if these three disagree.

Entry format: `## X.Y.Z (YYYY-MM-DD)`, followed by *Added / Changed / Fixed*
sections that name feature IDs where there are any.

---

## 2.0.0 (2026-09-29)

mdedit is now also a **library for embedding the editor** in other
programs (tabs, file lists, vaults), while the `mdedit` program works as
before. Major version: the library's API changed.

### Added
- **Embedding API** (`documentation/embedding.md`):
  - `shared::Shared`: settings, terminal capabilities, images, recent
    emoji and the link resolver, once per program.
  - `view::EditorView`: one open document (text, cursor, selection, undo,
    folds, scroll, search / replace / emoji prompts). `handle_key` returns
    an `Outcome`: `Consumed`, `Ignored` (the host's keys), `OpenLink`,
    `RequestOpen`, `RequestSaveAs`, `RequestClose`. Also `open`, `save`,
    `save_as`, `is_dirty`, `title`, `status_line`, `go_to_heading`.
  - `ui::EditorWidget`: draws a view into any area, with or without a
    status bar; prompts and the emoji picker stay inside the area; the
    cursor position and image areas are left in the view.
    `ui::apply_terminal_workarounds` is public, for the host's frame.
  - `resolver::Resolver`: how links, note embeds and images find their
    files; `RelativeResolver` is mdedit's own (next to the note).
- `tests/embedding.rs`: the API used the way a host would (an editor
  beside a file list, outcomes, a vault-style resolver, two views sharing
  settings), and a compiled example in the crate docs.
- `scripts/release.py`: the version bookkeeping for a release.

### Changed
- `app::App` is now just the mdedit program: one `Shared`, one
  `EditorView`, the file browser, the save prompt and exiting.
- The status bar shows the latest message before the key hints, so it's
  no longer cut off in an 80-column terminal (e.g. "Saved …").
- Double-size headings are off when the editor is narrower than the
  screen (the terminal sizes whole rows).
- An untitled document ends with a newline when saved.

## 1.8.1 (2026-09-28)

**Milestone 2 (links) is complete**: K-01 … K-09 and K-12 (K-10 dropped).

### Fixed
- A link to the note being edited (`[[this-note#Heading]]`,
  `[text](./this-note.md#heading)`) jumps to the heading within the
  document. It used to ask to save unsaved changes and reload the file.
- Auto-pair no longer panics if the cursor column is past the end of the
  line.

## 1.8.0 (2026-09-28)

### Added
- **K-08 autolinks**: `<https://…>`, `<mailto:…>` and `<me@example.com>`
  are shown without the angle brackets, underlined like URLs. HTML tags
  (`<b>`) and `a < b > c` are left alone.

## 1.7.0 (2026-09-27)

### Added
- **K-07 reference-style links**: `[text][ref]` and `[text][]` show the
  text as a link; definition lines `[ref]: url "title"` are dimmed, with
  the target in the link style. Ctrl+Enter on a reference link follows it
  through its definition (labels match ignoring case).

### Fixed
- Ctrl+Enter on `[text](Note.md "title")` or `[text](<My Note.md>)` tried
  to open a file named with the title or the angle brackets.

## 1.6.0 (2026-09-27)

### Added
- **K-06 Markdown links**: `[text](https://…)` shows the text, underlined
  blue like a URL; `[text](Note.md)` shows it light blue like a wiki link.
  Markup in the text works (`[**bold** link](…)`), and so do
  `[text](<My Note.md>)`, `[text](Note.md "title")` and `mailto:`. An image
  inside a line, `![alt](img.png)`, shows as `🖼 alt`.

## 1.5.0 (2026-09-27)

### Added
- **K-05 block ID**: `^block-id` at the end of a line (after a space) is
  dimmed when the cursor isn't on the line. `2^10`, a lone `^` and a `^id`
  in the middle of a line are left alone.

## 1.4.0 (2026-09-27)

### Added
- **K-03 link to heading** and **K-04 link to block**: `[[Note#Heading]]`
  is shown as `Note › Heading` and `[[Note#^block]]` as `Note › ^block`;
  a heading in the same note, `[[#Heading]]`, as just `Heading`. Inline
  embeds read the same (`⧉ Note › Heading`).

## 1.3.0 (2026-09-27)

### Added
- **Complete `mdedit --help`**: besides the command-line options it lists
  every key (moving and selecting, editing, search, view, links and files),
  every `config.toml` setting with its values and default, and where the
  config file and recent emoji are kept. `tests/project.rs` checks that
  every key, setting and option in the README is in the help, and that the
  help fits in 80 columns.

### Changed
- K-10 `obsidian://` URIs are dropped: they belong to the Obsidian app,
  not a single-file editor.

## 1.2.1 (2026-09-27)

### Changed
- Technology doc: lists syntect, ratatui-image and image, and the new
  modules (`selection`, `autopair`, `images`).

## 1.2.0 (2026-09-27)

### Added
- **E-04 image embeds**: a line that is just `![[photo.png]]` or
  `![alt](photo.png)` shows the picture in a frame under a title row
  (`🖼 photo.png · 640×480`), relative to the note's folder. Drawn with
  `ratatui-image`: kitty, sixel or iTerm2 graphics where the terminal
  supports them, colored half blocks (`▀`) elsewhere. Images are at most
  20 rows tall and fit the window's width, keeping their shape; decoded and
  encoded images are cached until the file changes. With the cursor on the
  line it shows the link. A missing file says "not found"; web images
  aren't downloaded, only named.
- **E-05 image size**: `![[photo.png|200]]` (200 pixels wide),
  `|200x100`, and `![alt|200](photo.png)`.
- Setting `images = "auto" | "kitty" | "sixel" | "iterm2" | "halfblocks" |
  "off"`. "auto" asks the terminal at startup (half a second at most).
  In Konsole, "auto" uses half blocks, because `ratatui-image` considers
  Konsole's sixel and kitty support unreliable; try "sixel" or "kitty".
- `example_mds/sunset.png`, shown in the feature showcase.

### Changed
- The terminal workarounds (repainting every cell, emoji fallbacks) leave
  image cells alone, so image data is never re-sent or changed.

## 1.1.0 (2026-09-27)

### Added
- **L-09 task types** and **L-10 custom task states** are marked done:
  they already worked through L-05's glyphs. `- [/]` in progress ◐,
  `- [>]` forwarded ➜ (not "in progress"), `- [-]` cancelled ☒,
  `- [!]` important ⚑. A task state is a single character: `[?]` shows as
  `?`, any other character as `[c]`, and `- [doing]` is plain list text.

### Changed
- Project plan and requirements: the task-type section no longer lists open
  decisions; `[>]` is "forwarded".

## 1.0.0 (2026-09-27)

**Milestone 1 (core editor) is complete.** All 52 M1 features are done:
text formatting, lists and tasks, blocks (quotes, callouts, code, tables),
the live-preview editing behaviors, and file handling (F-01 … F-07).

### Changed
- **V-01 show raw syntax under cursor** is done as designed: the whole
  cursor line (and every selected line) is shown raw. mdedit deliberately
  doesn't follow Obsidian's element-by-element reveal.

### Notes
- The scripted Konsole test (`cargo test --test konsole -- --ignored`) is
  to be re-run by hand; it opens a Konsole window.

## 0.54.1 (2026-09-27)

### Fixed
- Feature showcase: the settings paragraph split a code span across two
  lines, so its backticks showed raw; it's rewrapped and lists the new
  `auto_pair` and `undo_steps` settings.

### Changed
- Project plan: M1 counts updated (51 of 52 done; only V-01 is partial).

## 0.54.0 (2026-09-27)

### Changed
- **Ctrl+Z undoes up to 5 steps** (V-18; it was 1,000). A typed word, an
  Enter, a paste or a replace-all is one step. The number is a setting:
  `undo_steps = N` in `config.toml`, from 1 to 10000. When the saved text
  falls out of the history, undoing no longer clears the `[+]` mark.

## 0.53.0 (2026-09-27)

### Added
- **V-04 auto-pair**: typing `(`, `[`, `{` or a backtick inserts the
  closer after the cursor, and so does the second `*` of `**` (and `==`,
  `~~`). Typing the closer steps over it; Backspace between an empty pair
  deletes both. No pair is added right before a word, and ``` can still be
  typed for a code fence. `auto_pair = "off"` in `config.toml` turns it
  off.

## 0.52.0 (2026-09-27)

### Added
- **V-19 text selection**: Shift + arrows, Home / End and Page Up / Down
  select text; Ctrl+A selects everything; Esc or a plain move ends the
  selection. Selected lines are shown raw with the selection reversed.
  Typing (or Enter) replaces the selection, Backspace / Delete delete it,
  and Tab / Shift-Tab indent or outdent every selected line.
- **V-05 wrap selection**: with text selected, `*`, `_`, `~`, `=`, `` ` ``
  and `"` wrap it on both sides, and `(`, `[`, `{` wrap it with their
  closers. The text stays selected, so `*` twice makes it bold and `[`
  twice a wiki link.
- **V-09 paste URL over selection**: pasting an http(s) address over a
  selection makes `[selection](url)`; any other paste replaces the
  selection.
- Key scripts in the feature tests can use `<S-…>` (Shift) and
  `<paste:text>`.

## 0.51.1 (2026-09-27)

### Fixed
- Embedding a section (`![[Note#Heading]]`) treated a `# comment` inside a
  code block as a heading, which cut the section short, and couldn't find
  setext headings. Sections now use the document structure.

### Changed
- Heading detection lives in one place (`Structure::heading`), used by
  folding, heading sizes and spacing, `#Heading` links and embeds.
- `markdown.rs` is split into `markdown/` (`mod.rs` line parser, `inline.rs`,
  `render.rs`, `table.rs`) and `ui.rs` into `ui/` (`mod.rs` frame,
  `doc.rs` document layout, `popups.rs` dialogs). No behavior change.

## 0.51.0 (2026-09-27)

### Added
- **V-18 undo / redo**: Ctrl+Z undoes, Ctrl+Y (or Ctrl+Shift+Z where the
  terminal reports it) redoes. Typing and deleting undo a word at a time;
  Enter, a paste, a replace-all or a task toggle are one step each. The
  cursor goes back to where the change was. Undoing back to the saved text
  clears the `[+]` unsaved mark. The history (`src/history.rs`) keeps only
  the lines that changed, up to 1,000 steps, and starts fresh for each
  opened file.

## 0.50.3 (2026-09-27)

### Fixed
- **Pasting** inserts the text exactly as copied. Pastes used to arrive as
  keystrokes, so every newline acted like Enter (pasting `- a` / `- b` gave
  `- a` / `- - b`, and numbered lists were renumbered) and tabs indented
  the line. mdedit now turns on bracketed paste. In the search, replace,
  file name and emoji fields a paste types its first line; a paste never
  answers a yes/no question.

## 0.50.2 (2026-09-27)

### Fixed
- **F-07 safe save**: saving writes a temporary file next to the note and
  then replaces the note in one step, so a crash or a full disk can't leave
  a half-written file, and a failed save leaves the old file untouched.
  Saving through a symbolic link writes the linked file (the link stays),
  and the file keeps its permissions.
- Line endings are kept: a CRLF file stays CRLF, and a file without a final
  newline doesn't get one (it used to be converted to LF with a final
  newline on every save).

## 0.50.1 (2026-09-27)

### Fixed
- **Performance on large notes** (measured on a 10,000-line note):
  - Page Down re-analyzed the whole document for every screen row it
    moved: 117 ms → 5 ms. The block structure is now cached by the text
    (`blocks::analyze_cached`), so movement, folds and drawing share one
    analysis per change.
  - List-item and section folds were found with a quadratic scan: a
    3,000-line nested list took 300 ms per frame in a release build. Now one
    pass with a stack.
  - Every frame copied all syntax-highlighted code, even off-screen: now the
    cached blocks are shared.
  - Keys that arrive while a frame is drawn are all handled before the next
    frame, so held keys never lag.
  - Development builds (`cargo run`, the tests) use `opt-level = 1`: 15-20×
    faster than before.
- A panic no longer leaves the terminal in the kitty keyboard mode.

### Added
- `tests/performance.rs`: time budgets for large notes (typing, ↓,
  Page Down, a deep list, a long code block).

## 0.50.0 (2026-09-27)

### Added
- **V-17 find and replace**: Ctrl+H (or Ctrl+R, for terminals that send
  Ctrl+H as Backspace) opens a Find / Replace prompt in the status bar.
  Typing the search jumps to the first match; Tab switches fields; Enter
  replaces the match under the cursor and goes to the next; ↑/↓ and F3 skip
  a match; Ctrl+A replaces every match and says how many. Smart case like
  search; matches don't overlap.

## 0.49.0 (2026-09-27)

### Added
- **V-14 task type continues on Enter**: Enter on `- [.] log` starts
  `- [.] `, and the same for `[/]`, `[>]`, `[!]`, `[?]` …; after a finished
  task (`[x]`, `[X]`, `[-]`) the new task is open (`[ ]`). Works for
  numbered tasks too.

## 0.48.0 (2026-09-27)

### Added
- **V-13 source mode**: **Ctrl+V** switches between the live preview and
  source mode, where every line is raw Markdown (markers dimmed, headings
  colored) and nothing is folded, expanded or framed (tables, embeds,
  heading spacing, big headings). `SOURCE` shows in the status bar.
  Up/Down don't skip folded lines in source mode.
- **`-t` / `--source`** on the command line starts in source mode (see
  `mdedit -h`).
- `ui::render_app`: renders exactly what the app shows (feature snapshots
  use it).

### Changed
- **V-12 reading view dropped**: not needed; the live preview plus source
  mode cover it.
- The plan's section 2.5 status lines are now generated too (V-06 and V-07
  were already done but still listed as open there).

## 0.47.1 (2026-09-27)

### Fixed
- **Crash** (the window closed): inline code containing a multi-byte
  character, e.g. `` `▸ N lines` `` in the feature showcase, panicked when
  it came into view (as with Page Down), because the code-span search
  sliced text inside a character. Probably also the cause of the earlier
  "window disappears" reports.

### Added
- `tests/robustness.rs`: renders every example note and fixture at many
  widths with the cursor on every line, and fuzzes the inline parser and
  wrapper with 10,000 random strings of markup and multi-byte text. Both
  tests fail without the fix.

## 0.47.0 (2026-09-27)

### Added
- **V-06 / V-07 folding everything with Ctrl+K**: besides callouts, Ctrl+K
  folds a heading's section (up to the next heading of the same or a
  higher level) and a list item's sub-items and continuation lines. It
  acts on the innermost one around the cursor. A folded section or item
  ends with a dim `▸ N lines`, even while the cursor is on it, and the
  status bar says what was folded. Up/Down skip folded lines, and moving
  into them some other way unfolds them.
- `ui::render_document_folded` (snapshots can show folds made by keys).

## 0.46.0 (2026-09-27)

### Added
- **Heading colors** in `config.toml`: `heading1_color` … `heading6_color`
  take a color name (`red`, `light blue`, `dark-gray` …), `#rrggbb` or a
  256-palette number. They apply to rendered and raw headings, setext
  underlines and embedded notes, and are adapted to the terminal's colors
  like every other color. Unset levels keep their default; invalid values
  are reported in the status bar.

## 0.45.0 (2026-09-27)

### Added
- Command-line options (`src/cli.rs`): `--big-headings`,
  `--heading-size=auto|on|off`, `--help`, `--version`. Unknown options
  are reported (exit code 2).

### Changed
- **Bigger headings are off by default**: headings are normal size unless
  you start mdedit with `--big-headings` / `--heading-size=on|auto` or set
  `heading_size = "on"` (or `"auto"`) in `config.toml`. The command line
  wins over the config file.

## 0.44.0 (2026-09-27)

### Added
- **Terminal compatibility**: mdedit adapts to what the terminal can do.
  RGB colors become the nearest 256-palette or basic color where there's
  no true color. Terminals without Unicode (the Linux console, non-UTF-8
  locales) get ASCII look-alikes for box drawing, bullets, checkboxes and
  icons, and `?` for other characters (keeping their width). Settings
  `colors = "auto|truecolor|256|16"` and `glyphs = "auto|unicode|ascii"`
  override the detection. Compatibility table in
  `documentation/technology_and_skills.md`.

## 0.43.1 (2026-09-27)

### Changed
- `example_mds/feature_showcase.md` shows everything up to 0.43.0: embeds,
  search, Page Up/Down, opening at a section, bigger headings.
- **Way of working:** every release also updates the feature showcase
  (demonstrates the new feature and sets its frontmatter `version:`).
  `tests/project.rs` enforces the version, and the rule is in CLAUDE.md,
  the `mdedit-feature-workflow` skill and VERSION.md.

## 0.43.0 (2026-09-27)

### Added
- **R-16 heading sizes**: in terminals with DEC double-size lines
  (Konsole, xterm, WezTerm), level-1 headings are drawn double size (two
  rows tall) and level-2 double width, wrapped at half the width. Levels
  3-6 and other terminals keep the color/glyph style. A heading being
  edited is normal size. Setting `heading_size = "auto" | "on" | "off"`.
- `src/terminal.rs`: terminal capabilities detected from the environment
  (color depth, DEC line sizes, Unicode), the basis for adapting to any
  terminal.

## 0.42.0 (2026-09-27)

### Added
- **E-01 / E-02 embeds**: a line that is just `![[Note]]` shows that note,
  rendered in a `╭─ ⧉ Note` … `╰─` frame, and `![[Note#Heading]]` shows
  only that section. While the cursor is on the line, only the link is
  shown. Notes are resolved relative to the current file (no vault) and
  cached until they change on disk. Embeds inside an embedded note stay
  links (no loops). A missing note shows `⚠ … (not found)` (`src/embed.rs`).
- Inline `![[Note]]` (not on its own line) shows as `⧉ Note` in link color.

### Changed
- E-01 / E-02 moved from the Wrapper milestone back to mdedit (M3, done
  early); the wrapper keeps block embeds (E-03) and vault-wide lookup.

## 0.41.0 (2026-09-27)

### Added
- **V-16 search**: **Ctrl+F** opens a search prompt in the status bar,
  and the cursor jumps to the first match as you type. ↓ / F3 and
  ↑ / Shift+F3 go to the next / previous match; Enter keeps the position,
  Esc goes back to where you started. **F3 / Shift+F3** repeat the last
  search while editing. Smart case (a capital makes it case-sensitive),
  wraps around, shows "3/12", and opens a folded callout it lands in
  (`src/search.rs`).

## 0.40.0 (2026-09-27)

### Added
- **F-06 open at a section**: `mdedit "note.md#My Title"` opens with the
  cursor on that heading and the heading at the top of the screen;
  following a `[[Note#Heading]]` link lands the same way. Headings match
  ignoring case, or by letters and digits alone (`#my-title`, `#mytitle`).
  A file whose name contains `#` still opens as-is.

## 0.39.0 (2026-09-27)

### Added
- **V-15 Page Up / Page Down**: move the cursor a screen of rows (less
  one, for context) and scroll the view with it, keeping the column,
  skipping folded lines and counting wrapped rows.
- Key scripts (`tests/features.rs`) know `<PgUp> <PgDn> <F3> <S-F3>
  <Esc>` and run on a 80 × 10 screen.

## 0.38.0 (2026-09-27)

### Added
- **F-05 / K-12 follow link**: with the cursor on a link, **Ctrl+Enter**
  (or **Alt+Enter**, for terminals that can't report Ctrl+Enter) opens the
  linked Markdown file. It works for `[[Note]]`, `[[Note|alias]]`,
  `[[Folder/Note]]`, `[[Note#Heading]]` (jumps to the heading),
  `[[#Heading]]` (within this file) and `[text](file%20name.md)`. Targets
  are resolved relative to the current file, with `.md` added when needed;
  there's no vault. With unsaved changes a prompt says the link is being
  followed and offers Yes (save first) / No (discard and follow) /
  Cancel. Web links, missing files and "no link here" are reported in the
  status bar (`src/links.rs`).

## 0.37.1 (2026-09-27)

### Changed
- **Scope:** mdedit edits one Markdown file at a time and never knows
  about a vault. The vault features (K-11 unresolved links, K-13 link
  autocomplete, E-01 to E-03 embedding other notes, E-08 search, E-09
  Bases, V-10 tag/property autocomplete, C-02 Tasks queries) move to the
  new **Wrapper** milestone, for a future project that will wrap mdedit as
  its multi-tab editor (plan, section 6, lists the hooks mdedit should
  offer it). M2 becomes "Links", resolved relative to the current file.
- Requirement **F-05 / K-12**: Ctrl+Enter (or Alt+Enter) follows the
  link under the cursor, asking to save unsaved changes first. Not
  implemented yet.

## 0.37.0 (2026-09-27)

### Added
- **B-09 syntax highlighting**: fenced code with a known language
  (```` ```rust ````, ```` ```py ````, ```` ```js ```` …, by name,
  token or file extension) is colored with syntect's built-in syntaxes and
  the `base16-ocean.dark` theme (foreground colors only). Unknown or
  missing languages keep the plain code color. Results are cached by block
  text and the block being edited isn't highlighted: a 300-line block
  costs ~1.6 ms per frame in a release build (34 ms without the cache).
- New dependency: `syntect` (pure-Rust regex engine, no C library).

## 0.36.0 (2026-09-27)

### Added
- **B-13 wiki links inside tables**: an escaped pipe (`\|`) stays inside
  its cell, so `[[Note\|alias]]` in a table shows `alias` as a link, and
  `a \| b` shows `a | b`.

## 0.35.0 (2026-09-27)

### Added
- **B-12 table column alignment**: `:---` left, `:---:` centered and
  `---:` right, for header and body cells (`markdown::Align`,
  `table_aligns`).

## 0.34.0 (2026-09-27)

### Added
- **B-11 tables**: GFM tables (a header row, a `|---|` separator with the
  same number of cells, then rows with pipes) are drawn with box borders:
  bold header, columns as wide as their widest rendered cell (markup not
  counted), top and bottom borders as extra display rows. Rows aren't
  soft-wrapped. The whole table is raw while the cursor is inside it.
  Outer pipes are optional.

## 0.33.0 (2026-09-27)

### Added
- **B-10 indented code blocks**: lines indented 4+ columns (a tab = 4)
  after a blank line or at the start, outside lists, are shown as code
  (framed, literal, 4 columns of indent removed) and are raw as a whole
  while the cursor is inside. They can't interrupt a paragraph, and
  indented lines in lists stay list content.

## 0.32.0 (2026-09-27)

### Added
- **B-08 fenced code blocks** are complete: fences inside quotes and
  callouts (e.g. the ```` ```tasks ```` queries in daily notes) are framed
  as code after the quote bars, and their body is shown literally. The
  whole block is raw while the cursor is inside it. A quoted fence ends
  with the quote if it isn't closed (`QuoteCode`).

## 0.31.0 (2026-09-27)

### Added
- **B-05 foldable callouts**: `> [!type]-` starts collapsed and
  `> [!type]+` open, and the header shows ▸ / ▾. **Ctrl+K** folds or
  unfolds the callout under the cursor (from inside the body, the cursor
  moves to the header). Up/Down skip folded lines. If the cursor lands in
  a folded body some other way, that callout opens, so hidden text is
  never edited. Fold choices live in the editor only and reset to the
  `+`/`-` defaults when lines are added or removed.

## 0.30.0 (2026-09-27)

### Added
- **B-04 callout icons**: each callout title starts with an icon for its
  type, like Obsidian: ✎ note, ≡ abstract, ℹ info, ☑ todo, ✦ tip,
  ✔ success, ? question, ⚠ warning, ✘ failure, ϟ danger, ※ bug,
  ☰ example, ❝ quote. They're single-width symbols, not emoji, so Konsole
  lines them up (checked with `tests/konsole.rs`).
- **B-07 custom callouts** are complete: any `[!name]` gets the default
  color and the ✎ icon (Obsidian's CSS-based custom styling doesn't apply
  in a terminal).

## 0.29.0 (2026-09-27)

### Added
- **B-06 nested callouts**: `> > [!tip] Inner` inside a callout has its
  own color and title, and every bar takes the color of the callout at
  its depth (`LineContext::Quote { callouts }`, `markdown::render_quote`).

### Changed
- All quote lines now get a document-level context (a callout per depth),
  replacing `LineContext::CalloutBody` and `render_callout_body`.

## 0.28.0 (2026-09-27)

### Added
- **B-02 nested blockquotes**: `> > text` (or `>>text`) draws one bar per
  level, and continuation rows wrap under the text. `Block::Quote` now
  has a `depth` (`markdown::quote_prefix`).

## 0.27.0 (2026-09-27)

### Added
- **R-07 configurable done style**: `done_style = "grey"` in
  `~/.config/mdedit/config.toml` shows done tasks greyed out without
  strikethrough (Obsidian's look); `"strike"` is the default.
- mdedit's first settings file (`src/config.rs`): `key = "value"` lines and
  `#` comments. Bad lines are reported in the status bar and ignored.

### Changed
- `ui` renders through a `Doc` (lines + block structure + cursor +
  options); `markdown::render_with` / `render_source_with` take `Options`.

## 0.26.0 (2026-09-27)

### Added
- **L-07 paragraphs inside list items**: indented lines under a list item
  (also after a blank line) belong to it (`LineContext::ListContinuation`).
  They're drawn with the item's indent guides and lined up with its text,
  which is also their wrap indent. Unindented text ends the list.

## 0.25.0 (2026-09-27)

### Added
- **L-08 list renumbering**: after Enter, Backspace/Delete joining lines,
  Tab or Shift-Tab, the numbered list around the cursor is renumbered.
  A list counts from its first item's number (`3.` stays `3.`), each
  nesting level counts on its own, and blank lines between items keep
  the list together. A bullet at the same level or a switch between `.`
  and `)` starts a new sequence. Tab on a numbered item starts a nested
  list at 1. The cursor follows when a number gains a digit.

## 0.24.0 (2026-09-27)

### Added
- **L-06 ordered task lists**: `1. [ ] task` shows its number and
  checkbox (`1. ☐ task`). Ctrl+L toggles it, Ctrl+T turns `1. item` into
  a task, and Enter continues with the next number.

## 0.23.0 (2026-09-27)

### Added
- **L-02 ordered lists**: `1.` and `1)` items (1–9 digits), nested with
  tabs or spaces like bullets, shown with their number in the list color.
  Enter continues with the next number (`9)` → `10)`); Enter on an empty
  item ends the list.

### Changed
- List markers are now the marker text (`"-"`, `"12."`) instead of a char;
  Ctrl+T / Ctrl+L / Enter use the marker's real length.

## 0.22.0 (2026-09-27)

### Changed
- Emoji picker (T-14 / EP-04): **Ctrl+E** closes the picker as Esc does,
  so the same key opens and closes it. The picker footer says
  `Esc/^E close`.

## 0.21.0 (2026-09-27)

### Added
- **R-17 heading spacing**: a blank row appears above a heading that
  directly follows text, like Obsidian's space above headings. Display
  only, in rendered and raw view alike; cursor placement and scrolling
  count it (`Wrapped::pad_top`).

## 0.20.0 (2026-09-27)

### Added
- **T-13 emoji** is complete: every emoji sequence Konsole draws wider than
  it reports now has an on-screen fallback, so none of them swallows the
  text after it. Joined emoji → first emoji, flags → country letters
  (🇹🇷 → `TR`), tag flags → 🏴, skin tones → the emoji without its tone. The
  file keeps the real emoji (`ui::terminal_fallback`).
- Konsole test with a note of tricky emoji (flags, tag flag, joined,
  keycap, variation selectors, skin tone).

### Fixed
- Flags and skin-tone emoji in a document swallowed the following text in
  Konsole.
- Docs: the claim that Konsole draws skin tones 2 wide was wrong (its
  cursor query says 2, but the screen shows more).

## 0.19.0 (2026-09-27)

### Added
- **T-02 setext headings**: a paragraph line directly above a `===` line
  is a level-1 heading, and above a `---` line a level-2 heading
  (CommonMark). The underline is drawn as a `═══` / `───` rule in the
  heading's color, as wide as the heading. The pair shows raw together
  while the cursor is on either line. A `---` after a blank line, list
  item, quote or heading is still a horizontal rule.

### Changed
- T-12 fixture: the "two dashes aren't a rule" example now follows a
  blank line; directly under text, `--` is (correctly) a setext heading.

## 0.18.0 (2026-09-27)

### Added
- **T-05 bold + italic**: `***text***` and `___text___` are bold italic,
  and emphasis nests (`**a *b* c**`).

## 0.17.0 (2026-09-27)

### Added
- **T-04 italic** is complete: `_text_` is italic as well as `*text*`.
  `snake_case_words` stay as typed (no underscore emphasis inside words).

## 0.16.0 (2026-09-27)

### Added
- **T-03 bold** is complete: `__text__` is bold as well as `**text**`.
  Underscore markers can't be inside a word (`x__y__z` stays literal).

## 0.15.0 (2026-09-27)

### Added
- **T-09 backslash escapes**: `\*`, `\#`, `` \` ``, `\[` … show the
  character literally, with the backslash hidden, so it doesn't start
  markup, a tag or a link. Only ASCII punctuation can be escaped
  (`a\b` stays as typed), and an escaped marker never closes emphasis.

## 0.14.0 (2026-09-27)

### Added
- **T-08 inline code**: `` `code` `` shows in the code color with the
  backticks hidden, and nothing inside is parsed as markup. Longer runs
  (``` ``a ` b`` ```) can hold backticks, one space of padding on both sides
  is trimmed, and an unmatched backtick stays literal.

## 0.13.0 (2026-09-27)

### Added
- **T-07 highlight**: `==text==` shows black on yellow with the markers
  hidden; `a == b` isn't a highlight (the markers must hug their text).

## 0.12.0 (2026-09-27)

### Added
- **T-06 strikethrough**: `~~text~~` is struck through with the markers
  hidden; a single `~` isn't.

### Changed
- The inline parser is rewritten as a recursive parser driven by a table
  of markers (`DELIMITERS`). Markup now nests (`~~a **b** c~~`), a marker
  must hug its text (`~~ x ~~` stays literal), and a closer must be a run
  of exactly the marker's length.

## 0.11.2 (2026-09-27)

### Changed
- Docs: Konsole takes Ctrl+Shift+S for its own "Save Output As…" (a KDE
  dialog), so the key never reaches mdedit. Konsole does support the kitty
  keyboard protocol, so unbinding its shortcut makes Ctrl+Shift+S work.
  Ctrl+Alt+S always works (README, F-02 notes).

## 0.11.1 (2026-09-27)

### Changed
- Requirements:
  - **V-14 (M1)**: Enter on a task continues its type (`- [.] log` →
    `- [.] `); finished types (`[x]`, `[-]`) start a fresh `[ ]`.
  - **L-09 (M3)**: task types in progress (`[/]`, `[>]`), cancelled /
    dropped (`[-]`), important / priority (`[!]`).
  - **L-10 (M3)**: custom task text (`- [?]`, `- [doing]`).
  - Pending fixtures added; open decisions recorded in the plan (the
    meaning of `[>]`, a key to set the type, allowed word characters). Not
    implemented yet.

## 0.11.0 (2026-09-27)

### Added
- **EP-06 recently used emoji** (from Milestone 4, done early): with an
  empty search, the emoji picker starts with the emoji you inserted most
  recently (up to 32, newest first, no duplicates). They're remembered
  between sessions in `~/.config/mdedit/recent_emoji` (or
  `$XDG_CONFIG_HOME/mdedit/`). A typed search isn't affected. If saving the
  list fails, the emoji is still inserted and the status bar says so.

## 0.10.0 (2026-09-27)

### Added
- **T-14 emoji picker (Ctrl+E)**, EP-01 to EP-05: a popup with a search
  field and a 16-column emoji grid. Typing searches names and shortcodes
  with ranking: exact, prefix, word start, substring, then fuzzy (letters in
  order). The arrow keys move the highlight through the grid, Enter inserts
  the emoji at the cursor and closes, and Esc cancels. Emoji data comes from
  the `emojis` crate (`src/emoji.rs`).
- Konsole test for the picker (`tests/konsole.rs`): pages through the whole
  grid in a real Konsole and compares the screens.

### Changed
- Joined emoji and flags (regional-indicator and tag sequences) are left
  out of the picker, because Konsole draws them wider than measured and
  they broke the grid (found by the new Konsole test).
- **EP-06 to EP-12** (recently used, skin tones, categories, preview,
  mouse, `:shortcode:` autocomplete) moved to **Milestone 4**, which had
  nothing assigned until now.

## 0.9.2 (2026-09-27)

### Changed
- Requirements: added **T-14 emoji picker (Ctrl+E)** at the top of M1 text
  formatting. It has a search field, arrow-key navigation, and Enter inserts
  at the cursor. The spec is in `requirements/emoji_picker_requirements.md`
  (EP-01 to EP-04 as requested; EP-05 to EP-12 are options found in the
  Obsidian emoji-toolbar plugin and still need a decision). Pending fixture
  `T-14-emoji-picker.md` + `.keys` added. Not implemented yet.

## 0.9.1 (2026-09-27)

### Added
- **R-18 real-terminal test** (`tests/konsole.rs`, run with
  `cargo test --test konsole -- --ignored`): runs the binary inside Konsole
  via `scripts/konsole_drive.py`, scrolls a real note, reads the screen back
  over D-Bus and compares it with ratatui's drawing. Verified that it
  catches the original emoji artifact when the fix is disabled. This
  completes the M1 foundation (R-03, R-04, R-18, F-01 to F-04).

## 0.9.0 (2026-09-27)

### Added
- **F-04 Ctrl+O open**: with unsaved changes it first asks
  `[Y]es [N]o [C]ancel` (Yes saves first, Save As if untitled). Then comes a
  file picker (the Save As browser): folders first, then Markdown files,
  `..` to go up, Ctrl+H for all files, type to filter or type a path. The
  chosen file replaces the document (cursor at the top, Ctrl+S saves to
  it). Unreadable or non-UTF-8 files show an error in the picker. Opening
  the current file reloads it.
- Status bar: `^O open`.

### Changed
- F-04 requirement: after **No**, changes are discarded only when another
  file is actually opened. Esc in the picker returns to the document with
  nothing lost (the M1 "no silent data loss" rule).

## 0.8.0 (2026-09-27)

### Added
- **F-03 Ctrl+X exit**: exits straight away when everything is saved.
  Otherwise it asks `Save changes to <name>? [Y]es [N]o [C]ancel`. Yes saves
  (Save As for an untitled document) and exits; cancelling Save As or a
  failed write keeps you in the editor. No exits without saving. Cancel/Esc
  returns to editing.
- Status bar lists `^S save ^⌥S save as ^X exit`.

### Changed
- **Ctrl+Q no longer exits** (replaced by Ctrl+X, as the requirement says).

## 0.7.0 (2026-09-27)

### Added
- **F-01 Ctrl+S**: saves straight to the document's file with no prompt.
  An untitled document opens Save As (F-02) instead of just showing an
  error. On a write error the document stays marked unsaved and the error
  shows in the status bar.

## 0.6.0 (2026-09-27)

### Added
- **F-02 Save As** (Ctrl+Shift+S where the terminal supports the kitty
  keyboard protocol, Ctrl+Alt+S everywhere): a popup folder browser (`..`,
  folders, Markdown files) with a name field filled in with the current
  name or `untitled.md`. `.md` is added when there's no extension. Typed
  paths (`sub/note`, `~/notes/x.md`) work. Asks before overwriting. After
  saving, the document belongs to the new file. Esc cancels. The browser
  lives in `src/files.rs` and will be shared with Open (F-04).
- Integration tests for file operations (`tests/file_ops.rs`, real files in
  Cargo's temp dir).

### Changed
- `App.path` is now a `PathBuf`; `App` has a `mode` (editing or a dialog)
  and a `cwd` for untitled documents.

## 0.5.0 (2026-09-27)

### Added
- **R-03 / V-08 soft wrap** (`src/wrap.rs`): long lines wrap at word
  boundaries onto more screen rows, with a hanging indent so continuation
  rows line up with the text after the bullet, checkbox or quote bar. It
  works in both the rendered and the raw cursor-line views. Scrolling counts
  screen rows, and Up/Down move by screen row, keeping the column.
- Feature snapshots are rendered at 80 columns (`SNAPSHOT_WIDTH`).

### Changed
- `markdown::render` / `render_source` return a `Rendered` (line + hanging
  indent). `ui::render_document` takes a width and returns screen rows.

## 0.4.0 (2026-09-27)

### Added
- **R-18 grapheme-aware cursor**: Left/Right move and Backspace/Delete
  delete whole grapheme clusters, so joined emoji (`🧑‍🦳`) and combining
  accents take one keypress. Up/Down never leave the cursor inside a
  cluster. Uses `unicode-segmentation`.

## 0.3.0 (2026-09-27)

### Added
- **R-04 block parser** (`src/blocks.rs`): a document-level scan that gives
  each line its block context (frontmatter, code fence, callout body).
  Frontmatter and code blocks are *reveal groups*: raw as a whole while the
  cursor is inside them. Hand-written scanner; tree-sitter deferred (see
  R-04 for why).
- **B-08 fenced code blocks** (🟡): framed with `╭─ lang` / `│` / `╰─`, and
  the body is shown literally (no Markdown styling). Fences inside
  quotes/callouts are not handled yet.

### Changed
- **B-03 callouts** (✅): body lines now carry the callout's colored bar,
  not just the title (B-07 custom callouts too).
- `markdown::frontmatter_end` removed; frontmatter detection moved to
  `blocks`.

## 0.2.1 (2026-09-27)

### Changed
- Requirements: added **F-04 Ctrl+O: open file** to Milestone 1 file
  operations. If there are unsaved changes it first asks Yes/No/Cancel,
  then shows a file picker that shares its folder browser with Save As
  (`requirements/file_handling_requirements.md`, `documentation/project_plan.md`).
  Not implemented yet.

## 0.2.0 (2026-09-27)

### Added
- **Feature test suite** (`tests/features.rs`): one Markdown fixture per
  feature in `requirements/obsidian_features.md` (88 fixtures, 9 sections),
  approved `.expected` snapshots, `.keys` scripts for editing behaviors, and
  a generated `tests/TEST_MATRIX.md`. 27 features are checked and 61 are
  pending.
- **Project consistency tests** (`tests/project.rs`): the version in
  Cargo.toml, VERSION.md and README must agree.
- `scripts/check.sh`: format, lint and test gate.
- Project documentation: README, this file, `documentation/project_plan.md`,
  `documentation/technology_and_skills.md`, `CLAUDE.md` (way of working).
- Claude Code skills in `.claude/skills/`: `mdedit-feature-workflow`,
  `test-driven-development`, `rust-testing`, `rust-skills`. Reference copies
  of the Rust API Guidelines checklist and the Microsoft Pragmatic Rust
  Guidelines.

### Changed
- Split into a library crate (`src/lib.rs`) and a thin binary, so tests can
  reach the editor. Key handling moved from `main.rs` to `App::handle_key`
  (`src/app.rs`).
- Code reformatted with default `rustfmt`.

### Fixed
- B-04: `fail` and `missing` callouts are now red, as in Obsidian (they are
  aliases of `failure`). `example` is now purple and `important` green.

## 0.1.0 (2026-09-27)

The proof of concept. This version was assigned afterwards and covers all
work before the test suite.

### Added
- Terminal editor (ratatui) with Obsidian-style Live Preview: the cursor
  line shows raw Markdown, and every other line is shown formatted.
- Headings `#`–`######` (T-01); bullets `-` `*` `+` with nesting via spaces
  or tabs and indent guides (L-01, L-03).
- Tasks `- [ ]` / `- [x]` with strikethrough, and alternate states `[.]`
  `[/]` `[-]` `[>]` … (L-04, L-05). Ctrl+T creates a task and Ctrl+L closes
  or reopens it (V-11).
- List continuation on Enter; Tab/Shift-Tab nesting (V-02, V-03).
- Inline: `[[wiki links]]` with aliases, bare URLs, `#tags`, `**bold**`,
  `*italic*` (K-01, K-02, K-09, P-05, T-03, T-04).
- Blockquotes, callout titles, horizontal rules, dimmed frontmatter (B-01,
  B-03, T-12, P-01).
- Ctrl+S save, and Ctrl+Q quit with a guard against unsaved changes.

### Fixed
- Konsole rendering artifacts when scrolling near emoji: every cell is now
  repainted in place. Joined emoji (`🧑‍🦳`) are shown as their first emoji
  (R-18).
