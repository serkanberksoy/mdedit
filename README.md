# mdedit

**A terminal Markdown editor with Obsidian-style live preview.** Every line
is shown formatted: headings, checkboxes, links, tables, callouts, even
images. Only the line under the cursor switches to raw Markdown, so you
edit plain text and read a rendered note, both in the terminal. Written in
Rust with [ratatui](https://ratatui.rs).

```bash
mdedit note.md                # open (or create) a note
mdedit "note.md#My Heading"   # open it at a heading
mdedit --help                 # all options, keys and settings
```

Type to edit, move with the arrows. **Ctrl+S** saves, **Ctrl+X** exits,
**Ctrl+F** searches, **Ctrl+Z** undoes.

**Version:** 3.13.1 · **Stage:** Milestones 1 (core editor) and 2 (links) complete; next: M3 · [Version history](VERSION.md)

```
█ MARCH 14TH, 2026                    ← # heading, rendered
▎ Logs
• 09:00
│ ⦿ measured the back yard  📗 Project - Garden
- [x] clear the old pots ✅ 2026-03-14  ← cursor line: raw Markdown
☐ another task for 📗 Project - Garden
```

- **Live preview** like Obsidian: formatted everywhere except where you type.
- **Your Obsidian notes read well**: wiki links, callouts, task states,
  embeds, tags, frontmatter, tables, syntax-highlighted code, images.
- **A real editor**: undo, selection, find and replace, auto-pair, list
  continuation, folding, emoji picker, safe saving.
- **One file at a time, no vault**: links are relative to the note.
- **Works in any terminal**, with the most in Konsole, kitty, WezTerm and
  xterm.
- **Embeddable**: a library for programs with tabs and file lists.

Contents: [Quick start](#quick-start) · [Features](#features) ·
[Keys](#keys) · [Command line](#command-line) · [Settings](#settings) ·
[Using mdedit in another program](#using-mdedit-in-another-program) ·
[Roadmap](#roadmap) · [Development](#development)

## Quick start

Requires a Rust toolchain (1.94+).

```bash
cargo install --path .                       # installs `mdedit`
mdedit notes/today.md                        # open (or create) a note
mdedit example_mds/feature_showcase.md       # a tour of every feature
```

Inside mdedit, **Ctrl+S** saves, **Ctrl+X** exits and `mdedit --help` lists
everything.

## Features

Everything below is tested (83 of 92 tracked Obsidian features have
approved tests; see [tests/TEST_MATRIX.md](tests/TEST_MATRIX.md)). Open
[example_mds/feature_showcase.md](example_mds/feature_showcase.md) in
mdedit to try each one.

### Live preview

- Every line is rendered; the line with the cursor is raw Markdown, so the
  syntax is there exactly where you edit it.
- Multi-line blocks (frontmatter, code blocks, tables) turn raw as a whole
  when the cursor enters them. Selected lines are raw too.
- **Three modes**, cycled with Alt+V: the live preview, **source mode** (every line raw; `mdedit -t` starts in it) and **view mode** (read-only, every line rendered: move over rendered rows, Tab to a link, Enter or a click follows it).
- **Soft wrap**: long lines wrap at word boundaries, continuation rows line
  up under the text (also in lists and quotes), and ↑ / ↓ move by screen row.

### Text

| Markdown | Shown as |
|----------|----------|
| `# Heading` … `###### Heading`, `===` / `---` underlines | Colored, weighted headings with a glyph per level (`█ TITLE`, `▌ Sub` …); optionally double size in Konsole, xterm, WezTerm |
| `**bold**` `__bold__` `*italic*` `_italic_` `***both***` | Bold, italic, both; markup nests (`**bold *italic* inside**`) |
| `~~strike~~` `==highlight==` `` `code` `` | Struck through, highlighted, code color |
| `\*escaped\*`, `snake_case`, `2 * 3` | Left as typed |
| `---` | A horizontal rule |
| Emoji 😀 🧑‍🦳 🇹🇷 | Drawn correctly; where a terminal can't (Konsole: flags, skin tones, joined emoji), a readable stand-in |

### Lists and tasks

- Bullets (`-` `*` `+`), numbered lists (`1.` `1)`), nested with spaces or
  tabs, with `│` indent guides.
- **Enter continues the list**: next bullet, next number, a new task *of
  the same type*; Enter on an empty item ends the list. Numbers are
  renumbered as you edit.
- **Tab / Shift-Tab** nest and un-nest items.
- **Task states**, each with its own glyph:

  | `[ ]` | `[x]` | `[-]` | `[.]` | `[/]` | `[>]` | `[<]` | `[!]` | `[?]` | `[*]` |
  |---|---|---|---|---|---|---|---|---|---|
  | ☐ open | ☑ done | ☒ cancelled | ⦿ log | ◐ in progress | ➜ forwarded | ⏲ scheduled | ⚑ important | ? question | ★ star |

  Done tasks are struck through (or grey: `done_style = "grey"`).
  **Ctrl+T** makes a task, **Ctrl+L** closes or reopens it.

### Links

| Markdown | Shown as |
|----------|----------|
| `[[Note]]`, `[[Note\|alias]]` | `Note`, `alias` |
| `[[Note#Heading]]`, `[[#Heading]]`, `[[Note#^block]]` | `Note › Heading`, `Heading`, `Note › ^block` |
| `[text](https://…)`, `[text](Note.md)` | The text, underlined blue for the web, light blue for files |
| `[text][ref]` with `[ref]: url` | The text; the definition dimmed |
| `<https://…>`, `https://…`, `<me@example.com>` | Underlined links |
| `#tag`, `#type/project` | Tag chips (`#12` and `a#b` aren't tags) |
| `text ^block-id` | The ID dimmed |

**Ctrl+Enter** (or Alt+Enter) follows a link to a Markdown file next to the
note, asking to save changes first. A link to a heading lands with the
heading at the top; a link to a heading in the same note jumps there.

### Embeds and images

- `![[Note]]` or `![[Note#Heading]]` on its own line shows that note (or
  section) in a frame.
- `![[photo.png]]` or `![alt](photo.png)` shows the **picture**: with
  kitty, sixel or iTerm2 graphics where the terminal supports them, colored
  half blocks anywhere else. `|200` or `|200x100` sets the size in pixels.
  Web images show their address.
- Move the cursor onto the line to see the link instead.

### Blocks

- **Blockquotes**, nested (`> >`), with `│` guides.
- **Callouts** `> [!note] Title`: the title, an icon and a color per type
  (note, abstract, info, todo, tip, success, question, warning, failure,
  danger, bug, example, quote, and your own), nested callouts, and foldable
  ones (`> [!tip]-` starts folded, `+` open).
- **Code blocks**, fenced (```` ``` ```` or `~~~`) or indented, **syntax
  highlighted** by language; code blocks work inside callouts too.
- **Tables** with borders, column alignment (`:--`, `:-:`, `--:`), links and
  escaped pipes in cells.
- **Frontmatter** (`---` YAML at the top), dimmed.

### Editing

- **Undo / redo** (Ctrl+Z / Ctrl+Y), a word at a time, 5 steps by default
  (`undo_steps`). Undo back to the saved text and the `[+]` mark goes away.
- **Selection** with Shift + arrows, Home, End, Page Up / Down; Ctrl+A
  selects all. Typing replaces it; Tab indents its lines.
- **Wrap a selection**: `*` makes it italic, `*` again bold; `==` highlight,
  `[[` a wiki link, `(` `[` `{` `"` `` ` `` too.
- **Paste a URL over a selection** to make `[selection](url)`. Pastes go in
  exactly as copied (no list continuation, no indenting).
- **Auto-pair**: `(` `[` `{` `` ` `` and `**` `==` `~~` get their closer;
  typing the closer steps over it (`auto_pair = "off"` turns it off).
- **Emoji picker** (Ctrl+E): search by name or shortcode, recently used
  first, arrows and Enter.

### Search and navigation

- **Search** (Ctrl+F): the cursor jumps to the first match as you type and
  **every match on screen is highlighted**, the current one in red. ↑ / ↓
  or F3 / Shift+F3 go to the next / previous match; the status line counts
  them ("3/12"). A lowercase search ignores case.
- **Find and replace** (Ctrl+H or Ctrl+R): Enter replaces a match and moves
  on, Ctrl+A replaces all.
- **Folding** (Ctrl+K): a heading's section, a list item with sub-items or a
  callout; a folded item shows `▸ N lines`.
- **Page Up / Page Down**, and `mdedit "note.md#Heading"` opens a note at a
  heading.

### Files

- **Ctrl+S** saves; **Ctrl+Alt+S** (or Ctrl+Shift+S) saves as, with a folder
  browser; **Ctrl+O** opens a file; **Ctrl+X** exits. All of them ask
  before unsaved changes are lost.
- **Safe saving**: a temporary file replaces the note in one step, so a
  crash or a full disk never leaves half a note. Line endings (LF or CRLF),
  the final newline, permissions and symbolic links are kept.

### Terminals

- Adapts to the terminal: true color, 256 or 16 colors, and plain ASCII
  symbols for the Linux console (detected, or `colors` / `glyphs` in the
  settings).
- Uses the kitty keyboard protocol where available (Ctrl+Shift+S, …) and
  bracketed paste.
- Works around Konsole's wide-character quirks (no stale fragments while
  scrolling), checked by a scripted test in a real Konsole.
- Fast on large notes: a 10,000-line note stays responsive, and there are
  time budgets in the tests.

## Keys

| Key | Action |
|-----|--------|
| Arrows, Home, End | Move |
| Page Up / Page Down | Move a screen up or down |
| Enter | New line; continues bullets and tasks, and ends the list on an empty item |
| Tab / Shift-Tab | Nest / un-nest a list item or task |
| Ctrl+F | Search: jumps to the first match as you type, and every match on screen is highlighted (the current one in red); ↑/↓ previous/next, Enter keeps the position, Esc goes back (case-insensitive unless the search has a capital) |
| Ctrl+H (or Ctrl+R) | Find and replace: type the search, Tab to the replacement; Enter replaces the match and goes to the next, ↑/↓ skip, Ctrl+A replaces all, Esc closes |
| F3 / Shift+F3 | Next / previous match of the last search |
| Ctrl+Z / Ctrl+Y | Undo / redo, up to 5 steps (`undo_steps` setting); Ctrl+Shift+Z also redoes where the terminal reports it; typing undoes a word at a time |
| Shift + arrows / Home / End / PgUp / PgDn, Ctrl+A | Select text (selected lines are shown raw). Typing replaces the selection, Backspace / Delete delete it, Tab / Shift-Tab indent its lines |
| `*` `_` `~` `=` `` ` `` `"` `(` `[` `{` with a selection | Wrap the selection (`*` twice: bold); it stays selected |
| Paste a URL over a selection | Makes a link: `[selection](url)` |
| `(` `[` `{` `` ` `` and `**` `==` `~~` | Auto-pair: the closer is typed for you, typing it again steps over it, Backspace in an empty pair deletes both (`auto_pair = "off"` turns it off) |
| Ctrl+E | Emoji picker: type to search, arrows to choose, Enter inserts; Esc or Ctrl+E again closes it |
| Ctrl+T | Turn the line into a task (or add a new task below) |
| Ctrl+L | Close the task (`[x]`), or reopen a closed one |
| Ctrl+K | Fold / unfold what's under the cursor: a heading's section, a list item with sub-items, or a callout (`> [!type]-` starts folded). A folded item shows `▸ N lines` |
| Ctrl+V | Paste from the clipboard (read with wl-paste, xclip, xsel or pbpaste); the terminal's own paste works too |
| Alt+V | Cycle the modes: live preview → source mode (every line raw) → view mode (read-only, every line rendered) → live preview |
| Tab / Shift+Tab (view mode) | Go to the next / previous link (Enter or a click follows it; Esc edits again) |
| Ctrl+Enter / Alt+Enter | Follow the link under the cursor (`[[Note]]`, `[[Note#Heading]]`, `[text](file.md)`), relative to this file; asks to save unsaved changes first |
| Ctrl+O | Open a file: folder browser, type to filter or type a path; asks to save unsaved changes first |
| Ctrl+S | Save (an untitled document opens Save As) |
| Ctrl+Shift+S / Ctrl+Alt+S | Save As: pick a folder and a name (`.md` is added); asks before overwriting. In Konsole, Ctrl+Shift+S is Konsole's own "Save Output As…" unless you unbind it; use Ctrl+Alt+S |
| Ctrl+X | Exit; with unsaved changes asks Save? [Y]es / [N]o / [C]ancel |

## Command line

```bash
mdedit note.md                     # open (or create) a file
mdedit "note.md#My Title"          # open at a heading (shown at the top)
mdedit -t note.md                  # start in source mode (every line raw)
mdedit --big-headings note.md      # double-size level-1/2 headings (Konsole, xterm, WezTerm)
mdedit --heading-size=auto note.md # … only where the terminal supports it
mdedit --help                      # every option, key and setting
mdedit --version
```

## Settings

Optional, in `~/.config/mdedit/config.toml` (or `$XDG_CONFIG_HOME/mdedit/`):

```toml
# How done tasks look: "strike" (default) or "grey" (like Obsidian)
done_style = "grey"
# Auto-pair brackets, backticks and **, ==, ~~: "on" (default) or "off"
auto_pair = "on"
# Images: "auto" (ask the terminal), "kitty", "sixel", "iterm2",
# "halfblocks" (any true-color terminal) or "off" (title only).
# In Konsole "auto" uses half blocks; try "sixel" or "kitty".
images = "auto"
# How many steps Ctrl+Z can undo: 1 to 10000 (default 5)
undo_steps = 5
# Spaces Tab indents a list item (and per list level): 1 to 8 (default 2)
indent_width = 2
# Colors: "auto" (detected), "truecolor", "256" or "16"
colors = "auto"
# Symbols: "auto" (detected), "unicode" or "ascii" (e.g. the Linux console)
glyphs = "auto"
# Larger level-1/2 headings with the terminal's double-size lines:
# "off" (default), "on", or "auto" (on in Konsole, xterm, WezTerm).
# The command line (--big-headings, --heading-size=...) overrides this.
heading_size = "off"
# Heading colors, levels 1-6: a name (red, light blue, dark gray ...),
# "#rrggbb", or a 256-palette number. Unset levels keep their default.
heading1_color = "magenta"
heading2_color = "#00afaf"
heading3_color = 214
```

A bad line is reported in the status bar and ignored. Recently used emoji
are kept in `~/.config/mdedit/recent_emoji`.

## Using mdedit in another program

mdedit is also a library. A program with tabs, a file list or a vault can
embed the editor:

- one `EditorView` per open document;
- drawn with `EditorWidget` into any part of the screen;
- keys the editor doesn't handle come back as outcomes (`OpenLink`,
  `RequestClose` …);
- links, embeds and images are found through the program's own `Resolver`.

See [documentation/embedding.md](documentation/embedding.md).

## Roadmap

| Milestone | Theme | Status |
|-----------|-------|--------|
| M1 | Core editor: formatting, lists, blocks, editing, files | ✅ complete (1.0.0) |
| M2 | Links | ✅ complete (1.8.1) |
| M3 | Embeds, metadata, math, diagrams, footnotes, comments | In progress: images done |

mdedit edits **one Markdown file at a time** and has no notion of a vault.
Vault features (link autocomplete, vault-wide search, queries) belong to a
program that embeds mdedit. The plan is in
[documentation/project_plan.md](documentation/project_plan.md).

## Development

Requires a Rust toolchain (1.94+, edition 2024).

```bash
scripts/check.sh                          # fmt + clippy + all tests (run before every commit)
cargo test                                # all tests
cargo test --test features                # feature suite (one test per Markdown fixture)
cargo test --test features 05-blocks      # one section of the feature suite
UPDATE_SNAPSHOTS=1 cargo test --test features   # approve snapshots + regenerate TEST_MATRIX.md
cargo test --test konsole -- --ignored    # the real-Konsole check (opens a window)
cargo run -- example_mds/feature_showcase.md
```

### Way of working

1. **TDD:** every feature starts with a failing test, usually its fixture's
   `.expected` snapshot written by hand. Red, then green, then refactor.
2. **Automated tests:** every feature has a fixture in `tests/fixtures/`
   (checked by `cargo test`), plus unit, integration, robustness (fuzzing),
   performance and embedding tests.
3. **Release notes:** after every feature, `scripts/release.py` bumps the
   version and adds the `VERSION.md` entry; the README and
   `example_mds/feature_showcase.md` are updated. `cargo test` checks that
   they agree.

Details: [CLAUDE.md](CLAUDE.md) · [tests/README.md](tests/README.md) ·
[documentation/technology_and_skills.md](documentation/technology_and_skills.md)

### Project layout

```
src/                 editor source (library + the mdedit binary)
tests/               feature suite, fixtures, snapshots, integration and consistency tests
requirements/        feature list with milestones and status, parity and file-handling requirements
documentation/       project plan, embedding guide, technology & skills, reference guidelines
example_mds/         sample notes: the feature showcase, a daily note, images
scripts/             check.sh (the gate), release.py, the Konsole test driver
.claude/skills/      Claude Code skills used on this project
```
