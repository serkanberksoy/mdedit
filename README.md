# mdedit

A terminal Markdown editor with **Obsidian-style Live Preview**: every line
is shown formatted, except the line under the cursor, which switches to raw
Markdown so you can edit it. Built in Rust with
[ratatui](https://ratatui.rs).

**Version:** 2.1.0 · **Stage:** Milestones 1 (core editor) and 2 (links) complete; next: M3 · [Version history](VERSION.md)

```
█ MARCH 14TH, 2026                    ← # heading, rendered
▎ Logs
• 09:00
│ ⦿ measured the back yard  📗 Project - Garden
- [x] clear the old pots ✅ 2026-03-14  ← cursor line: raw Markdown
☐ another task for 📗 Project - Garden
```

## Features

What works today (72 of 89 tracked Obsidian features have approved tests;
see [tests/TEST_MATRIX.md](tests/TEST_MATRIX.md)):

| Area | Supported |
|------|-----------|
| Headings | `#` … `######` and setext (`===` / `---` underlines), shown with color, weight and glyphs by level, with a blank row above headings that follow text. Optionally (`--big-headings` / `heading_size`), level 1 double size and level 2 double width in Konsole, xterm and WezTerm |
| Lists | `-` `*` `+`, nested with spaces or tabs, `│` indent guides, Enter continues the list |
| Layout | Soft wrap: long lines wrap at word boundaries and continuation rows line up under the text; Up/Down move by screen row |
| Tasks | `- [ ]` / `- [x]` (done tasks struck through), alternate states `[.]` `[/]` `[-]` `[>]` `[!]` … |
| Emoji | Ctrl+E picker: recently used first, search by name or shortcode (fuzzy), arrows to choose, Enter inserts at the cursor. Emoji Konsole can't draw (flags, skin tones, joined) show a readable fallback on screen |
| Images | `![[photo.png]]` and `![alt](photo.png)` on their own line show the picture (E-04), with kitty, sixel or iTerm2 graphics where the terminal supports them and colored half blocks elsewhere; `\|200` or `\|200x100` sets the size in pixels (E-05). Web images show their address |
| Inline | `**bold**`/`__bold__`, `*italic*`/`_italic_`, `***both***`, `~~strike~~`, `==highlight==`, `` `code` ``, `\` escapes (all nest), `[[wiki links]]` and `[[link\|alias]]`, bare URLs, `#tags` |
| Links | `[[Note]]`, `[[Note\|alias]]`, `[[Note#Heading]]` shown as `Note › Heading` (`[[#Heading]]` as `Heading`), `[[Note#^block]]`, block IDs `^id` (dimmed), Markdown links `[text](url)` and `[text](Note.md)` (the text shown), reference links `[text][ref]` with dimmed `[ref]: url` definitions, autolinks `<https://…>`, bare URLs; Ctrl+Enter follows them |
| Blocks | Blockquotes, callouts `> [!note]` (title and body colored by type), fenced code blocks, horizontal rules |
| Embeds | `![[Note]]` / `![[Note#Heading]]` on its own line shows that note (or section) in a frame, relative to this file; the link while the cursor is on it |
| Metadata | YAML frontmatter shown dimmed; raw while the cursor is inside it |
| Files | Open (Ctrl+O), Save (Ctrl+S), Save As (Ctrl+Shift+S / Ctrl+Alt+S), Exit (Ctrl+X), each asking before unsaved changes are lost |

Planned work is in [documentation/project_plan.md](documentation/project_plan.md)
(M1 core editor → M2 links → M3 embeds, metadata, math → M4 emoji picker
extras → M5 plugins).

mdedit edits **one Markdown file at a time** and has no notion of a vault.
Vault features (link autocomplete, embedding other notes, vault queries)
are for a separate project that will wrap mdedit as its multi-tab editor.

## Usage

```bash
cargo run -- path/to/note.md      # open (or create) a file
cargo run -- "note.md#My Title"   # open at a heading (it's shown at the top)
cargo run -- --big-headings note.md   # larger level-1/2 headings (Konsole, xterm, WezTerm)
cargo run -- -t note.md           # start in source (text) mode
cargo run -- --help                # every option, key and setting
cargo run --release -- note.md    # fully optimized build (dev builds use opt-level 1)
```

| Key | Action |
|-----|--------|
| Arrows, Home, End | Move |
| Page Up / Page Down | Move a screen up or down |
| Enter | New line; continues bullets and tasks, and ends the list on an empty item |
| Tab / Shift-Tab | Nest / un-nest a list item or task |
| Ctrl+F | Search: jumps to the first match as you type, and every match on screen is highlighted (the current one in red); ↑/↓ previous/next, Enter keeps the position, Esc goes back (case-insensitive unless the search has a capital) |
| Ctrl+H (or Ctrl+R) | Find and replace: type the search, Tab to the replacement; Enter replaces the match and goes to the next, ↑/↓ skip, Ctrl+A replaces all, Esc closes |
| Ctrl+Z / Ctrl+Y | Undo / redo, up to 5 steps (`undo_steps` setting); Ctrl+Shift+Z also redoes where the terminal reports it; typing undoes a word at a time |
| Shift + arrows / Home / End / PgUp / PgDn, Ctrl+A | Select text (selected lines are shown raw). Typing replaces the selection, Backspace / Delete delete it, Tab / Shift-Tab indent its lines |
| `*` `_` `~` `=` `` ` `` `"` `(` `[` `{` with a selection | Wrap the selection (`*` twice: bold); it stays selected |
| Paste a URL over a selection | Makes a link: `[selection](url)` |
| `(` `[` `{` `` ` `` and `**` `==` `~~` | Auto-pair: the closer is typed for you, typing it again steps over it, Backspace in an empty pair deletes both (`auto_pair = "off"` turns it off) |
| F3 / Shift+F3 | Next / previous match of the last search |
| Ctrl+E | Emoji picker: type to search, arrows to choose, Enter inserts; Esc or Ctrl+E again closes it |
| Ctrl+V | Switch between the live preview and source mode (every line raw) |
| Ctrl+K | Fold / unfold what's under the cursor: a heading's section, a list item with sub-items, or a callout (`> [!type]-` starts folded). A folded item shows `▸ N lines` |
| Ctrl+T | Turn the line into a task (or add a new task below) |
| Ctrl+L | Close the task (`[x]`), or reopen a closed one |
| Ctrl+Enter / Alt+Enter | Follow the link under the cursor (`[[Note]]`, `[[Note#Heading]]`, `[text](file.md)`), relative to this file; asks to save unsaved changes first |
| Ctrl+O | Open a file: folder browser, type to filter or type a path; asks to save unsaved changes first |
| Ctrl+S | Save (an untitled document opens Save As) |
| Ctrl+Shift+S / Ctrl+Alt+S | Save As: pick a folder and a name (`.md` is added); asks before overwriting. In Konsole, Ctrl+Shift+S is Konsole's own "Save Output As…" unless you unbind it; use Ctrl+Alt+S |
| Ctrl+X | Exit; with unsaved changes asks Save? [Y]es / [N]o / [C]ancel |

## Using mdedit in another program

mdedit is also a library: a program with tabs, a file list or a vault can
embed the editor (one `EditorView` per document, drawn with
`EditorWidget` into any part of the screen, with its own link resolver).
See [documentation/embedding.md](documentation/embedding.md).

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

Recently used emoji are kept in `~/.config/mdedit/recent_emoji`.

## Development

Requires a Rust toolchain (1.94+, edition 2024).

```bash
scripts/check.sh                          # fmt + clippy + all tests (run before every commit)
cargo test                                # all tests
cargo test --test features                # feature suite (one test per Markdown fixture)
cargo test --test features 05-blocks      # one section of the feature suite
UPDATE_SNAPSHOTS=1 cargo test --test features   # approve snapshots + regenerate TEST_MATRIX.md
```

### Way of working

1. **TDD:** every feature starts with a failing test, usually its fixture's
   `.expected` snapshot written by hand. Red, then green, then refactor.
2. **Automated tests:** every feature has a fixture in `tests/fixtures/`
   (checked by `cargo test`) plus unit tests.
3. **Release notes:** after every feature, bump the version in
   `Cargo.toml` and update `VERSION.md` and this README. `cargo test`
   checks that they agree.

Details: [CLAUDE.md](CLAUDE.md) · [tests/README.md](tests/README.md) ·
[documentation/technology_and_skills.md](documentation/technology_and_skills.md)

## Project layout

```
src/                 editor source (lib + binary)
tests/               feature suite, fixtures, snapshots, consistency tests
requirements/        feature list with milestones and status, parity and file-handling requirements
documentation/       project plan, technology & skills, reference guidelines
example_mds/         real Obsidian notes used for manual testing
.claude/skills/      Claude Code skills used on this project
```
