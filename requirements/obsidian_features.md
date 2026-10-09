# Obsidian Markdown Features: Master List

Every Markdown feature Obsidian supports, from CommonMark, GitHub Flavored
Markdown (GFM) and Obsidian's own extensions, plus the syntax from core
plugins and from the community plugins used in our notes. Use it to assign
features to milestones.

**Scope:** mdedit edits one Markdown file at a time and never knows about
a vault. Features that need other notes (a vault index, other notes'
content, vault-wide queries) have the milestone **Wrapper**. They belong to
a separate future project that wraps mdedit as its (multi-tab) editor.

**Milestones** are assigned. See `documentation/project_plan.md` for what
each milestone contains and the order of work.

**Now** is what mdedit supports today:
- ✅ done
- 🟡 partial (see Notes)
- ⬜ not started

**Effort** is a rough estimate:
- **S**: single line, can be parsed and styled line by line.
- **M**: spans several lines, so it needs block-level parsing (see
  R-04 in `obsidian_parity_requirements.md`).
- **L**: needs a vault index, file loading, or graphics/terminal protocol work.

---

## 1. Text formatting

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| T-14 | Emoji picker | Ctrl+E → search → arrows → Enter inserts at cursor | ✅ | M | M1 | Ctrl+E popup: search name/shortcode with fuzzy ranking, arrows move in the grid, Enter inserts at the cursor (EP-01…EP-05). EP-06…EP-12 are in M4; see `requirements/emoji_picker_requirements.md` |
| T-01 | Headings (ATX) | `#` … `######` | ✅ | S | M1 | Size shown by color and glyph |
| T-02 | Headings (Setext) | `Title` + `===` / `---` underline | ✅ | M | M1 | `===` → H1, `---` → H2 under a paragraph line; underline drawn as a matching rule; revealed as a pair |
| T-03 | Bold | `**text**`, `__text__` | ✅ | S | M1 | `**` and `__`; `_` markers can't be inside a word |
| T-04 | Italic | `*text*`, `_text_` | ✅ | S | M1 | `*` and `_`; `snake_case` stays literal; `2 * 3` isn't italic |
| T-05 | Bold + italic | `***text***` | ✅ | S | M1 | `***` and `___`; nested emphasis combines |
| T-06 | Strikethrough | `~~text~~` | ✅ | S | M1 | Markers hidden; nests with other markup |
| T-07 | Highlight | `==text==` | ✅ | S | M1 | Black on yellow; markers hidden |
| T-07a | Highlight colors | `==🔴text==` (🔴 🟠 🟡 🟢 🔵 🟣) | ✅ | S | M1 | The emoji colors the highlight and isn't shown; named colors (a palette changes them); `markdown::HIGHLIGHT_COLORS` for hosts |
| T-08 | Inline code | `` `code` `` | ✅ | S | M1 | Code color, backticks hidden, content literal; multi-backtick runs |
| T-09 | Backslash escapes | `\*`, `\#`, `\|` … | ✅ | S | M1 | ASCII punctuation only; escaped markers don't open or close markup |
| T-10 | Paragraphs / blank lines | blank line separates paragraphs | ✅ | S | M1 | |
| T-11 | Line breaks | single newline; `  ` or `\` at line end; `<br>` | ✅ | S | M1 | Terminal: every source line is its own row anyway |
| T-12 | Horizontal rule | `---`, `***`, `___` | ✅ | S | M1 | |
| T-13 | Emoji / Unicode | literal characters | ✅ | S | M1 | Konsole fallbacks on screen only: joined → first emoji, flags → letters (TR), tag flags → 🏴, skin tones → base emoji; see R-18 |

## 2. Lists and tasks

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| L-01 | Unordered list | `-`, `*`, `+` | ✅ | S | M1 | |
| L-02 | Ordered list | `1.`, `1)` | ✅ | S | M1 | Number shown in the list color; Enter continues the numbering |
| L-03 | Nested lists | indent with tab or spaces | ✅ | S | M1 | `│` indent guides |
| L-04 | Task list | `- [ ]`, `- [x]` | ✅ | S | M1 | Ctrl+T creates, Ctrl+L closes |
| L-05 | Alternate checkbox states | `- [.]`, `- [/]`, `- [-]`, `- [>]`, `- [!]` … | ✅ | S | M1 | Obsidian renders these per theme |
| L-06 | Ordered task list | `1. [ ] task` | ✅ | S | M1 | Number and checkbox shown; toggle, Ctrl+T and Enter keep the numbering |
| L-07 | Paragraphs inside list items | indented lines under an item | ✅ | M | M1 | Continuation lines keep the item's guides, align with its text and wrap under it; unindented text ends the list |
| L-08 | List renumbering | reorders `1. 2. 3.` after edits | ✅ | M | M1 | After Enter, joins, Tab/Shift-Tab: per level, from the first item's number; blank lines keep the list |
| L-09 | Task types: in progress, forwarded, cancelled, important | `- [/]` in progress · `- [>]` forwarded · `- [-]` cancelled / dropped · `- [!]` important / priority | ✅ | S | M3 | ◐ ➜ ☒ ⚑, through L-05; `[>]` is forwarded (the Obsidian convention) |
| L-10 | Custom task state | `- [?]`, `- [a]`: any single character inside the brackets | ✅ | S | M3 | Only a single character is a task state: `[?]` shows as `?`, other characters as `[c]`; `- [doing]` is plain list text |

## 3. Links and references

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| K-01 | Wiki link | `[[Note]]` | ✅ | S | M2 | Brackets hidden |
| K-02 | Wiki link with alias | `[[Note\|alias]]` | ✅ | S | M2 | |
| K-03 | Link to heading | `[[Note#Heading]]`, `[[#Heading]]` | ✅ | S | M2 | Shown as `Note › Heading`; a heading in the same note as `Heading` |
| K-04 | Link to block | `[[Note#^block-id]]` | ✅ | S | M2 | Shown as `Note › ^block-id` (like K-03) |
| K-05 | Block ID | `text ^block-id` at end of line | ✅ | S | M2 | Dimmed when the cursor isn't on the line; only at the end of a line, after a space |
| K-06 | Markdown link | `[text](https://…)`, `[text](Note.md)` | ✅ | S | M2 | The text is shown: web links underlined blue, files light blue; `<path with spaces>` and titles allowed; inline `![alt](img)` as `🖼 alt` |
| K-07 | Reference-style link | `[text][ref]` + `[ref]: url` | ✅ | M | M2 | `[text][ref]` and `[text][]` shown as links; definitions dimmed; Ctrl+Enter follows through the definition (labels ignore case) |
| K-08 | Autolink | `<https://…>` | ✅ | S | M2 | Also `<mailto:…>` and `<me@example.com>`; shown without the angle brackets |
| K-09 | Bare URL | `https://…` | ✅ | S | M2 | Underlined; clickable via OSC 8 still open (R-11) |
| K-10 | Obsidian URI | `obsidian://open?…` | ⬜ | S | Dropped | Not needed (decided 2026-09-27): `obsidian://` URIs belong to the Obsidian app, not a single-file editor |
| K-11 | Unresolved link styling | link to a note that doesn't exist | ✅ | L | M2 | 3.12.0: a wiki link whose target the resolver can't find is dimmed (relative to the file here; a host's `Resolver` decides in a vault) |
| K-12 | Follow link | Ctrl+Enter (or Alt+Enter) on a link | ✅ | L | M2 | Ctrl+Enter / Alt+Enter: wiki and Markdown links, #Heading jumps; relative to the current file (no vault); save prompt first; 3.2.0: web links and links to missing files come back to the host (`Outcome::OpenUrl`, `Outcome::MissingLink`) |
| K-13 | Link autocomplete | suggestions after typing `[[` | ⬜ | L | Wrapper | Wrapper project: needs a vault index of note names |

## 4. Embeds (transclusion)

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| E-01 | Embed note | `![[Note]]` | ✅ | L | M3 | Relative to the current file (no vault); framed; the link while the cursor is on it; cached until the file changes |
| E-02 | Embed heading section | `![[Note#Heading]]` | ✅ | L | M3 | The section under that heading, up to the next heading of the same or a higher level |
| E-03 | Embed block | `![[Note#^block-id]]` | ✅ | L | M3 | 3.7.0: the paragraph or list item a `^id` ends (an id on its own line: the block above it, e.g. a table); `[[Note#^id]]` goes to its line. The host's resolver finds the note |
| E-04 | Embed image | `![[img.png]]`, `![alt](url)` | ✅ | L | M3 | Terminal: kitty/sixel/iTerm image protocols, or a `🖼 img.png` placeholder |
| E-05 | Image size | `![[img.png\|200]]`, `\|200x100`, `![alt\|200](url)` | ✅ | L | M3 | |
| E-06 | Embed audio / video | `![[clip.mp3]]`, `![[clip.mp4]]` | ⬜ | S | M3 | Terminal: placeholder only |
| E-07 | Embed PDF | `![[doc.pdf]]`, `![[doc.pdf#page=3]]` | ⬜ | S | M3 | Terminal: placeholder only |
| E-08 | Search query embed | ```` ```query ```` block | ⬜ | L | Wrapper | Wrapper project: runs a vault search |
| E-09 | Bases embed | `![[view.base]]`, ```` ```base ```` block | ⬜ | L | Wrapper | Wrapper project: Bases are vault database views |

## 5. Blocks

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| B-01 | Blockquote | `> text` | ✅ | S | M1 | |
| B-02 | Nested blockquote | `> > text` | ✅ | S | M1 | One bar per level; wraps under the text |
| B-03 | Callout | `> [!note] Title` | ✅ | M | M1 | Title colored; body bar in the callout's color (via `blocks`) |
| B-04 | Callout types | note, abstract/summary/tldr, info, todo, tip/hint/important, success/check/done, question/help/faq, warning/caution/attention, failure/fail/missing, danger/error, bug, example, quote/cite | ✅ | S | M1 | Color and icon per type (✎ ≡ ℹ ☑ ✦ ✔ ? ⚠ ✘ ϟ ※ ☰ ❝), single-width symbols |
| B-05 | Foldable callout | `> [!note]+` open, `> [!note]-` collapsed | ✅ | M | M1 | `-` folded / `+` open, ▸/▾ on the header, Ctrl+K toggles; Up/Down skip folded lines |
| B-06 | Nested callout | `> > [!tip]` inside a callout | ✅ | M | M1 | Each depth's bar in its own callout's color; inner title in its color |
| B-07 | Custom callout types | any `[!name]` (styled via CSS in Obsidian) | ✅ | S | M1 | Any `[!name]`: default cyan and ✎ icon (Obsidian's default); custom styling via CSS doesn't apply in a terminal |
| B-08 | Fenced code block | ```` ``` ```` or `~~~` | ✅ | M | M1 | Framed, body literal, raw while the cursor is inside; also inside quotes and callouts |
| B-09 | Syntax highlighting | ```` ```rust ```` | ✅ | M | M1 | syntect, base16-ocean.dark; by name, token or extension; cached per block |
| B-10 | Indented code block | 4 spaces / tab | ✅ | M | M1 | After a blank line, outside lists; framed like fenced code; raw as a block while editing |
| B-11 | Tables | `\| a \| b \|` + `\|---\|` | ✅ | M | M1 | Box borders, bold header, columns fit the widest rendered cell; raw while editing |
| B-12 | Table column alignment | `:---`, `:---:`, `---:` | ✅ | M | M1 | Left / center / right from the separator row, header included |
| B-13 | Wiki link inside table | `[[Note\|alias]]` → `[[Note\\|alias]]` | ✅ | M | M1 | `\\|` stays in the cell; `[[Note\\|alias]]` shows the alias |

## 6. Metadata

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| P-01 | Frontmatter / properties | `---` YAML `---` at top of file | ✅ | M | M3 | 3.6.0: each property as its key and typed value; the `---` lines as rules; raw while the cursor is in it |
| P-02 | Property types | text, list, number, checkbox, date, date & time | ✅ | M | M3 | 3.6.0: text (links and tags styled), a list as chips, a number, a checkbox ☑ / ☐, a date or date and time |
| P-03 | Special properties | `tags`, `aliases`, `cssclasses` | 🟡 | M | M3 | 3.6.0: `tags` as tags, `aliases` and `cssclasses` as chips; what they do is the host's (blackglass uses aliases) |
| P-04 | Collapse properties | `▸ Properties (n)` | ⬜ | M | M3 | R-05 |
| P-05 | Inline tags | `#tag`, `#nested/tag` | ✅ | S | M3 | One color for all tags; per-prefix colors open (R-12) |
| P-06 | Tags in frontmatter | `tags: [a, b]` or a YAML list | ✅ | M | M3 | 3.6.0: `tags: [a, b]`, `tags: a b` and a YAML list, each as a tag |

## 7. Math, diagrams, footnotes, comments, HTML

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| X-01 | Inline math | `$e = mc^2$` | ✅ | M | M3 | Terminal: convert LaTeX to Unicode (`e = mc²`) |
| X-02 | Block math | `$$ … $$` | ✅ | M | M3 | Unicode approximation, or show as code |
| X-03 | Mermaid diagrams | ```` ```mermaid ```` | ⬜ | L | M3 | Terminal: show as code, or ASCII rendering |
| X-04 | Footnote reference | `text[^1]` | ✅ | S | M3 | 3.5.0: `[label]` in the footnote color (no superscript digits: they have no ASCII fallback) |
| X-05 | Footnote definition | `[^1]: note` | ✅ | M | M3 | 3.5.0: `[label]` then its text; not a link definition |
| X-06 | Inline footnote | `text^[inline note]` | ✅ | S | M3 | 3.5.0: `[text]` in the footnote color |
| X-07 | Comment (inline) | `%%hidden%%` | ✅ | S | M3 | 3.5.0: dimmed italic, the `%%` hidden (Obsidian hides comments in reading view; here they stay visible, dimmed) |
| X-08 | Comment (block) | `%%` … `%%` across lines | ✅ | M | M3 | 3.5.0: from a `%%` line to the next, dimmed; raw as a block while editing |
| X-09 | Inline HTML | `<u>`, `<sub>`, `<sup>`, `<kbd>`, `<mark>`, `<br>` | ⬜ | S | M3 | Map to terminal styles where possible |
| X-10 | Block HTML | `<details>`, `<div>`, `<iframe>`, … | ⬜ | M | M3 | Terminal: show raw or dimmed |

## 8. Community plugin syntax used in our notes

| ID | Feature | Syntax | Now | Effort | Milestone | Notes |
|----|---------|--------|-----|--------|-----------|-------|
| C-01 | Tasks: emoji fields | `✅ 2026-06-29`, `📅 due`, `⏳ scheduled`, `🛫 start`, `🔁 every week`, `⏫🔼🔽` priority | 🟡 | S | M5 | Shown as text; could be styled as chips. Done date on Ctrl+L is R-08 |
| C-02 | Tasks: query block | ```` ```tasks ```` | ⬜ | L | Wrapper | Wrapper project: queries run over the vault (R-09) |
| C-03 | Dataview | ```` ```dataview ````, inline `key:: value` | ⬜ | L | M5 | mdedit: style `key:: value` fields only; running Dataview queries is the wrapper project's job |

## 9. Live Preview editing behaviors

Not syntax, but part of what makes Obsidian feel the way it does.

| ID | Feature | Behavior | Now | Effort | Milestone | Notes |
|----|---------|----------|-----|--------|-----------|-------|
| V-01 | Show raw syntax under cursor | formatted everywhere else | ✅ | S | M1 | By design the whole cursor line is raw (and every selected line); mdedit doesn't follow Obsidian's element-by-element reveal |
| V-02 | List / task continuation | Enter continues, Enter on an empty item ends the list | ✅ | S | M1 | |
| V-03 | Indent / outdent | Tab / Shift-Tab on list items | ✅ | S | M1 | 3.1.0: the width is a setting, `indent_width` (1 to 8 spaces, default 2); list levels and indent guides follow it |
| V-04 | Auto-pair | typing `(`, `[`, `` ` ``, `**`, `==` inserts the closing pair | ✅ | S | M1 | Closers step over, Backspace deletes an empty pair, no pair right before a word; `auto_pair = "off"` in config.toml |
| V-05 | Wrap selection | select text, type `*` → `*text*` | ✅ | M | M1 | Shift + arrows select (V-19); `*`, `_`, `~`, `=`, backtick and `"` wrap, `(`, `[`, `{` wrap with their closers; the text stays selected |
| V-06 | Fold headings | collapse a section under a heading | ✅ | M | M1 | Ctrl+K folds a heading's section; `▸ N lines` shows it's folded |
| V-07 | Fold lists | collapse a list item's children | ✅ | M | M1 | Ctrl+K folds a list item's sub-items and continuation lines; `▸ N lines` shows it's folded |
| V-08 | Soft wrap | long lines wrap with a hanging indent | ✅ | M | M1 | R-03: word wrap with hanging indent (rendered and raw views); Up/Down move by screen row |
| V-09 | Paste URL over selection | turns the selection into `[selection](url)` | ✅ | M | M1 | http(s) URLs; other pastes replace the selection |
| V-10 | Tag / property autocomplete | suggestions after `#` / in frontmatter | ⬜ | L | Wrapper | Wrapper project: suggestions come from the whole vault |
| V-11 | Toggle checkbox | click or key flips `[ ]` ↔ `[x]` | ✅ | S | M1 | Ctrl+L |
| V-12 | Reading view | fully rendered, read-only mode | ✅ | S | M1 | View mode (3.0.0): Alt+V cycles live preview → source → view; every line rendered, a row cursor over rendered rows (embeds and code block results too), Tab / Enter / click follow links, Esc edits |
| V-13 | Source mode | plain Markdown, no rendering | ✅ | S | M1 | Alt+V (since 3.0.0; Ctrl+V and Ctrl+Shift+V paste); every line raw (markers dimmed, headings colored), no folds/embeds/tables; `-t` starts in it |
| V-15 | Page Up / Page Down | PgUp / PgDn | ✅ | S | M1 | A screen of rows minus one; view scrolls with it; keeps the column |
| V-16 | Search | Ctrl+F; F3 / Shift+F3 next / previous | ✅ | M | M1 | Ctrl+F jumps as you type; ↓/↑ or F3/Shift+F3 next/previous; Enter keeps, Esc goes back; smart case; wraps |
| V-20 | Highlight search matches | every match on screen while Ctrl+F / Ctrl+H is open | ✅ | S | M1 | Found in the text as shown (rendered headings, hidden markup); the match at the cursor in a stronger color; cleared when the prompt closes |
| V-19 | Text selection | Shift + arrows / Home / End / Page Up / Page Down, Ctrl+A selects all | ✅ | M | M1 | Typing replaces the selection, Backspace / Delete delete it, Tab indents its lines; selected lines are shown raw |
| V-18 | Undo / redo | Ctrl+Z undoes, Ctrl+Y (or Ctrl+Shift+Z) redoes | ✅ | M | M1 | Up to 1000 steps (`undo_steps` in config.toml); word-sized steps for typing; undoing back to the saved text clears the unsaved mark |
| V-17 | Find and replace | Ctrl+H; Enter replaces the match and goes to the next, Ctrl+A replaces all | ✅ | M | M1 | Two fields (Tab switches), jumps as you type, smart case like search |
| V-14 | Task type continues on Enter | Enter on `- [.] log` → `- [.] ` | ✅ | S | M1 | Keeps the type (`.` `/` `>` `!` `?` …); after `[x]`/`[-]` a fresh `[ ]` |

---

## Out of scope for a Markdown editor (Milestone: M5)

- Canvas (`.canvas` JSON files), graph view, backlinks pane, outline pane.
  These are vault/app features rather than Markdown syntax. The outline and
  backlinks could become terminal side panels later.
- Publish/Sync, themes and CSS snippets.
