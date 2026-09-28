# Obsidian Rendering Parity Requirements

Derived from comparing the POC editor (now in `src/`) with Obsidian's Live Preview on a
real journal note (a note of the same shape is `example_mds/daily_note.md`). Everything the POC doesn't
support is listed here. The goal is for real notes from an existing Obsidian
vault to read about as well in the terminal as they do in Obsidian.

Status: ✅ implemented in the POC · 🟡 partially implemented · ⬜ open

Priority: **P1** blocks daily use on real notes · **P2** clearly visible gap ·
**P3** nice to have

---

## 1. Summary of observed differences

| # | Element | Obsidian | POC before | POC now |
|---|---------|----------|------------|---------|
| 1 | YAML frontmatter | Collapsible "Properties" panel | Raw `---` / `key: value` lines | Dimmed properties with `┄` separators; the whole block shows raw while the cursor is inside it |
| 2 | Heading size | Real font sizes, color per level | Color, weight and a bar glyph | Unchanged (a terminal can't change font size) |
| 3 | Wiki links `[[target\|alias]]` | Brackets hidden, alias shown, link color | Raw `[[...]]` | Brackets hidden, alias shown, light blue |
| 4 | Unresolved wiki links | Different color and underline | n/a | Not distinguished |
| 5 | URLs | Underlined, link color | Plain text | Underlined blue |
| 6 | Tab-indented nested lists | Nested, with vertical indent guides | Tabs not recognized, shown as flat raw `- ...` | One tab = one nesting level, `│` guides |
| 7 | Custom checkbox `- [.]` | Circled-dot icon (theme) | Raw `- [.]` | `⦿`, plus other alternate states (see R-06) |
| 8 | Tags `#type/project` | Colored "pill", color varies by tag | Raw text | Colored background, one color for all tags |
| 9 | Long lines | Soft-wrapped, hanging indent aligned with text after the bullet | Cut off at the terminal edge | Unchanged |
| 10 | Closed task `- [x]` | Filled checkbox, text greyed, **no** strikethrough (theme) | `☑` + strikethrough | Unchanged. Strikethrough was an explicit requirement, so we keep it |
| 11 | Callouts `> [!todo]+ Title` | Colored box with icon and title, collapsible | Raw `> ...` | Title in the callout's color with a colored `┃` bar; body shown as quote lines |
| 12 | ```` ```tasks ```` query blocks | Run by the Tasks plugin and shown as a live task list | Raw | Raw (inside a quote bar) |
| 13 | `**bold**` / `*italic*` | Styled, markers hidden | Raw | Styled, markers hidden |
| 14 | Horizontal rule `---` | Thin line | Raw `---` | `────` line |
| 15 | Spacing around headings | Extra space above headings | Only the blank lines in the source | Unchanged |
| 16 | Emoji / ZWJ sequences (`🧑‍🦳`) | Correct | Stale fragments while scrolling; `🧑‍🦳 Robin` shown as `🧑‍🦳obin` in Konsole | Fixed: full in-place repaint each frame; joined emoji shown as their first emoji (see R-18) |

---

## 2. Requirements

### Block structure

- **R-01 Tab indentation** ✅ P1
  Leading tabs count as indentation. One tab = one nesting level (two spaces
  also = one level). Enter keeps the line's exact leading whitespace, and
  Tab/Shift-Tab indent with a tab if the line is already tab-indented.
  In the raw cursor-line view a tab shows as 4 columns, and the cursor
  position accounts for this.

- **R-02 Indent guides** ✅ P2
  Nested list items and tasks show a dim `│` guide for each level above them,
  like Obsidian's indentation guides.

- **R-03 Soft wrap with hanging indent** ✅ P1
  Lines longer than the terminal width wrap onto more screen rows. The
  continuation rows line up with the start of the item's text (after the
  guides and bullet or checkbox), not with column 0. This requires:
  - mapping between source lines and screen rows, for scrolling and cursor
    placement;
  - Up/Down moving by screen row inside a wrapped line (as Obsidian does);
  - the raw cursor-line view wrapping at the same width.

  ✅ Done in `src/wrap.rs`: greedy word wrap, where the space at a break isn't
  drawn and a word longer than a row is hard-broken; wide characters are
  never split. The hanging indent is the marker width (`│ ☐ `, `┃ `, or
  `- [ ] ` in the raw view), capped at half the width. Scrolling counts
  screen rows. Up/Down keep the screen column across rows and lines.

- **R-04 Multi-line block parsing** ✅ P1
  Some blocks span several lines: frontmatter, fenced code, callout bodies.
  They need a document-level parse, not line by line.
  ✅ `src/blocks.rs` scans the whole document each frame and gives every
  line a context (frontmatter, fence open/body/close, callout body), plus
  *reveal groups*: frontmatter and code blocks show raw as a whole while
  the cursor is inside them. Fences follow CommonMark (`` ` ``/`~`, 3+
  chars, closed by the same char and at least the same length, up to 3
  spaces of indent, unclosed fences run to the end).
  **Decision:** a hand-written scanner rather than tree-sitter-markdown.
  It's O(lines) per frame, which is instant for note-sized files, has no
  C dependency, and is easy to test. Revisit if profiling shows it's slow
  or inline parsing needs a real syntax tree. Tables and folding will add
  their contexts here.

- **R-05 Frontmatter as properties** ✅ P2 / ⬜ P3
  ✅ A leading `---` … `---` block is shown dimmed as `key: value` rows, and
  as raw text only while the cursor is inside it (the whole block reveals
  together).
  ⬜ Collapse it to a single `▸ Properties (n)` row, with a key to expand it.

### Tasks

- **R-06 Alternate checkbox states** ✅ P1
  `- [c]` with any single char `c` is a task. Rendering:

  | Source | Glyph | Meaning |
  |--------|-------|---------|
  | `[ ]` | ☐ | open |
  | `[x]` `[X]` | ☑ + strikethrough | done |
  | `[-]` | ☒ + strikethrough | cancelled |
  | `[.]` | ⦿ | log entry (as used in the Logs section) |
  | `[/]` | ◐ | in progress |
  | `[>]` | ➜ | forwarded |
  | `[<]` | ⏲ | scheduled |
  | `[!]` | ⚑ (bold) | important |
  | `[?]` | ? | question |
  | `[*]` | ★ | star |
  | anything else | `[c]` | shown as it is |

  Ctrl+L sets any non-done state to `[x]`, and sets `[x]` back to `[ ]`.

- **R-07 Configurable done style** ✅ P3
  Choose between strikethrough (default, current behavior) and
  greyed-out without strikethrough (Obsidian's look).
  ✅ Set `done_style = "grey"` (or `"strike"`) in
  `~/.config/mdedit/config.toml` (or `$XDG_CONFIG_HOME/mdedit/`). This is
  mdedit's first setting (`src/config.rs`: `key = "value"` lines, `#`
  comments; bad lines show a warning in the status bar). Cancelled tasks
  (`[-]`) stay struck through either way.

- **R-08 Tasks-plugin done date** ⬜ P2
  When Ctrl+L closes a task, optionally append ` ✅ YYYY-MM-DD` (the Obsidian
  Tasks plugin's format, already used in the example notes). Reopening removes
  it.

- **R-09 Tasks query blocks** ⬜ P3
  ```` ```tasks ```` blocks (`not done`, `due before …`, `sort by …`,
  `group by …`) run in Obsidian. At minimum show them as a collapsed,
  labelled block (`⚙ tasks query`). Actually running the queries across a
  vault is out of scope for the POC.

### Inline elements

- **R-10 Wiki links** ✅ P1 / ⬜ P2
  ✅ `[[target]]` shows as `target`, and `[[target|alias]]` as `alias`, in
  link color with the brackets hidden. This also covers the daily-note
  navigation line, which becomes `<< yesterday || tomorrow >>`.
  ⬜ Show unresolved links (target note doesn't exist in the vault)
  differently, as Obsidian does. This needs a vault root and a file index.
  ⬜ Follow a link with a key (e.g. Ctrl+Enter or `gd`) to open the target note.

- **R-11 URLs** ✅ P2 / ⬜ P3
  ✅ `http(s)://…` shows underlined in link color.
  ⬜ Emit OSC 8 hyperlinks so supporting terminals (Konsole, kitty, WezTerm)
  make them clickable.

- **R-12 Tags** ✅ P2 / ⬜ P3
  ✅ `#tag` and `#nested/tag` (preceded by start of line or whitespace, not all
  digits) show with a colored background.
  ⬜ Color by tag prefix (`#type/*` vs `#on/*` look different in Obsidian).
  Pad each tag with a space on either side so it reads as a pill.

- **R-13 Bold / italic** ✅ P2
  `**bold**` and `*italic*` show styled with the markers hidden. `_italic_`,
  `__bold__`, `~~strike~~`, `==highlight==` and `` `code` `` are ⬜ P2.

### Blocks

- **R-14 Callouts** ✅ P2
  ✅ `> [!kind]± Title` shows the title (or the kind in upper case if there's
  no title) in a color per kind: todo/info/note blue, tip/success green,
  danger/error red, warning/question yellow, quote grey.
  ✅ The whole callout body's bar is the same color as its header (via R-04).
  ✅ `+`/`-` are honored (B-05): `-` starts collapsed, and **Ctrl+K** toggles
  the callout under the cursor. The header shows ▸/▾. Since 0.47.0 Ctrl+K
  also folds heading sections (V-06) and list items with sub-items (V-07).
  ✅ Nested callouts (B-06) and icons (B-04).

- **R-15 Horizontal rules** ✅ P3
  `---`, `***` and `___` (3 or more characters) show as a dim line.

### Headings and typography

- **R-16 Heading size** ✅ P3
  Most terminals can't change font size, so size stays shown through color,
  weight and bar glyphs. Optionally use kitty's text-sizing protocol (OSC 66)
  for real larger headings in terminals that support it, and fall back to
  the current style everywhere else.
  ✅ Done with DEC line sizes instead of kitty's protocol: level-1 headings
  use double-size lines (`ESC # 3` / `ESC # 4`, two rows tall), level-2
  double-width lines (`ESC # 6`), wrapped at half the width. Enabled
  where supported: Konsole (checked with a screenshot), xterm, WezTerm
  (`src/terminal.rs`); elsewhere headings keep the color and glyph style. A
  heading being edited is normal size. **Off by default** (0.45.0): turn it
  on with `--big-headings` / `--heading-size=on|auto` or `heading_size =
  "on" | "auto"` in `config.toml`.

- **R-17 Heading spacing** ✅ P3
  Optionally show one blank "virtual" row above headings that have no blank
  line before them in the source. Display only: the file isn't changed.
  ✅ Always on for now (there's no settings file yet). Applies to ATX and
  setext headings, in the rendered and the raw view (so the text doesn't
  jump while you edit the heading). The cursor and scrolling account for the
  extra row.

- **R-18 Wide character / emoji rendering** ✅ P1
  ✅ Konsole leaves stale fragments when a normal character replaces an
  emoji, because ratatui skips rewriting the emoji's second cell and expects
  the terminal to have blanked it. You see this while scrolling, as lines
  switch between raw and formatted and their text shifts sideways. Fixed by
  repainting every cell in place each frame (`CellDiffOption::AlwaysUpdate`).
  ✅ Konsole draws ZWJ emoji sequences (`🧑‍🦳`) wider than their 2 measured
  cells, which swallows the text after them (`🧑‍🦳 Robin` came out as
  `🧑‍🦳obin`). Fixed by showing only the first emoji of the sequence; the
  file is not changed.
  ✅ The same happens with flags and skin tones (found by
  `tests/konsole.rs`). On screen only: 🇹🇷 → `TR`, 🏴 + tag characters → 🏴,
  👍🏽 → 👍 (`ui::terminal_fallback`). Note that Konsole's cursor-position
  query reports these as 2 wide even though it draws them wider, so only
  the real-screen comparison is reliable.
  ✅ The cursor moves, and Backspace/Delete delete, by grapheme cluster
  (`unicode-segmentation`): one press passes `🧑‍🦳` or `é` (e + combining
  accent). Up/Down snap to the start of a cluster.
  ✅ A repeatable real-terminal test, `tests/konsole.rs`, run with
  `cargo test --test konsole -- --ignored`. `scripts/konsole_drive.py` runs
  the real binary on a pty inside Konsole and types the keys (Konsole blocks
  sending keys over D-Bus). The screen is read back with `qdbus …
  getAllDisplayedTextList` and compared with ratatui's own drawing for the
  same keys and size. With the repaint fix disabled, it reproduces the
  original `withAlt` bug. The vt100 unit test can't catch this, because
  vt100 clears both halves of a wide character itself.

---

## 3. Suggested next order

1. R-03 soft wrap (the biggest remaining readability gap on real notes)
2. R-04 multi-line parsing (unblocks fenced code, callout bodies and R-09)
3. R-08 done date on Ctrl+L
4. R-10 unresolved links and following links (needs a vault concept)
5. R-13 remaining inline styles, R-12 per-prefix tag colors
