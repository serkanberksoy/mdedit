---
title: mdedit feature showcase
status: every implemented feature, in one note
tags: [showcase, markdown]
reviewed: true
created: 2026-06-29
version: 3.16.0
---
# mdedit Feature Showcase

Move the cursor through this note: every line is shown formatted, except the
line under the cursor, which switches to raw Markdown. Jump to a section with
Ctrl+Enter on these links: [[#Text formatting]] · [[#Lists and tasks]] ·
[[#Links and tags]] · [[#Embeds]] · [[#Quotes and callouts]] · [[#Code]] ·
[[#Tables]] · [[#Navigation and search]]

Open this note at any section from the command line:
`mdedit "feature_showcase.md#Tables"` (the heading lands at the top).

## Headings

Optionally, in Konsole, xterm and WezTerm, level-1 headings are double size
and level-2 headings double width: start with `mdedit --big-headings` or set
`heading_size = "on"`. By default (and while you edit a heading) they're
normal size with their colors and glyphs. Each level's color can be set in
`config.toml`, e.g. `heading1_color = "red"`, `heading2_color = "#ff8800"`
or `heading3_color = 214`.

# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6

Setext heading, level 1
=======================

Setext heading, level 2
-----------------------

A heading directly after text gets a blank row above it:
## Spaced heading

## Text formatting

This is **bold**, __also bold__, *italic*, _also italic_ and ***bold italic***.
This is ~~struck through~~, this is ==highlighted==, and this is `inline code`.
Highlights come in colors: ==🔴red==, ==🟠orange==, ==🟡yellow==, ==🟢green==,
==🔵blue== and ==🟣purple== (the emoji at the start picks it).
Markup nests: **bold with *italic* inside**, ~~struck with **bold** inside~~.
Code is literal: `**not bold** [[not a link]] #not-a-tag`, even ``with ` inside``.
Escapes show the character: \*not italic\*, \#not-a-tag, \[\[not a link\]\].
snake_case_words and 2 * 3 * 4 stay as typed; so does a == b.

This line is long enough to show soft wrap: it keeps going past the edge of a normal terminal window, so it wraps onto the next row instead of being cut off, and Up/Down move one screen row at a time.

Emoji: 😀 📕 ✅ 👍. Emoji Konsole can't draw show a stand-in on screen:
flags 🇹🇷 🇪🇬 (country letters), skin tones 👍🏽, joined emoji 🧑‍🦳 Robin.
Try the emoji picker: Ctrl+E, type to search, arrows, Enter.

---

## Lists and tasks

Ctrl+K on a list item with sub-items (or anywhere in a section, or on its
heading) folds it; a folded item ends with `▸ N lines`. Ctrl+K again
unfolds it.

- Bullet with `-`
* Bullet with `*`
+ Bullet with `+`
  - Nested with spaces
    - Deeper
	- Nested with a tab
- Long list items wrap under their text, not under the bullet, so a paragraph in a list stays readable at any width.
- An item with a paragraph
  continuation line, lined up with the item's text

  and a second paragraph after a blank line

1. First (Enter continues the numbering)
2. Second
   1. Nested list, counted on its own
   2. Tab / Shift-Tab renumber
   3.  third nested
3. Third

Tab indents by 2 spaces; set `indent_width = "4"` in the settings for
4 (the indent guides follow it).

1) Parenthesis style
2) Second

- [ ] Open task (Ctrl+L closes it, Ctrl+T makes a task)
- [.] Enter at the end of a log entry starts another log entry (the task type continues; after a done task Enter starts an open one)
- [x] Done task
- [-] Cancelled
- [.] Log entry
- [/] In progress
- [>] Forwarded
- [<] Scheduled
- [!] Important
- [?] Question (a task state is one character: `- [doing]` is just text)
- [*] Star
	- [ ] Nested task
1. [ ] Numbered task
2. [x] Numbered done task

## Footnotes and comments

A claim with a footnote[^1], a named one[^source], and one written in
place^[an inline footnote]. %%A comment: dimmed, and hidden from readers
of the note in Obsidian.%%

%%
A comment block, across lines:
move the cursor into it to edit it.
%%

[^1]: The first footnote's text.
[^source]: A named footnote.

## Links and tags

- Wiki link: [[daily_note]] (Ctrl+Enter opens it)
- With an alias: [[daily_note|a daily note]]
- To a heading in another note: [[sample#Lists]] (shown as `sample › Lists`)
- To a heading in this note: [[#Tables]]
- To a note that doesn't exist: [[no-such-note]] (dimmed)
- Markdown links: [the sample note](sample.md) (Ctrl+Enter follows it) and
  [the Rust site](https://www.rust-lang.org); **markup** works in [the *text*](sample.md)
- Bare URL: https://github.com/oliveryh/obsidian-emoji-toolbar
- Autolinks: <https://www.rust-lang.org> and <someone@example.com>
- Reference links: [the sample note][sample] and [Rust][] (Ctrl+Enter
  follows them through their definitions below this list)
- A block ID at the end of a line is dimmed ^showcase-id
- Tags: #mdedit #type/showcase #on/testing
- Not tags: issue #12, a#b

[sample]: sample.md "The sample note"
[rust]: https://www.rust-lang.org

## Embeds

A line that is just an embed shows the other note (relative to this file).
Move the cursor onto it to see the link instead.

![[sample#Lists]]

A block (`#^id`: the list item that ends with ` ^second`, with its
sub-items):

![[sample#^second]]

A whole note:

![[sample]]

A missing note:

![[no-such-note]]

Inline, an embed is just a link: see ![[sample]] for more.

An image (drawn with your terminal's image support, or colored half blocks;
`images = "sixel"` or `"kitty"` in `config.toml` forces a method, `"off"`
shows only the title):

![[sunset.png]]

At a width of 120 pixels (`|120`, or `|120x60` for both):

![[sunset.png|120]]

A web image isn't downloaded, just named:

![A web picture](https://example.com/picture.png)

## Quotes and callouts

> A plain quote
> across two lines
> > nested one level
> > > and two levels

> [!note] Note
> Every callout type has its own color and icon.

> [!abstract] Abstract

> [!info] Info

> [!todo] Todo

> [!tip] Tip

> [!success] Success

> [!question] Question

> [!warning] Warning

> [!failure] Failure

> [!danger] Danger

> [!bug] Bug

> [!example] Example

> [!quote] Quote
> — someone quotable

> [!my-own-type] Custom types get the default color and ✎

> [!note] Outer callout
> outer body
> > [!tip] Nested callout
> > inner body

> [!tip]+ Foldable, open by default (Ctrl+K on it to fold)
> This body can be folded.

> [!warning]- Foldable, folded by default (Ctrl+K to open)
> This body is hidden until you unfold it.
> Up/Down skip it while it's folded.

> [!todo] A callout with code inside
> ```tasks
> not done
> due today
> ```

## Code

```rust
// Syntax highlighting for known languages
fn main() {
    let greeting = "hello";
    println!("{greeting}, world");
}
```

```python
def greet(name: str) -> str:
    return f"hello, {name}"  # comment
```

```
A fence without a language: plain code color.
**Markup** [[inside]] code is #literal.
```

~~~
Tilde fences work too.
~~~

An indented code block follows (4 spaces, after a blank line):

    indented code
    shown like fenced code

## Tables

| Feature | Status | Version |
|:--------|:------:|--------:|
| **Tables** | ✅ | 0.34.0 |
| Alignment | ✅ | 0.35.0 |
| Links in cells | [[daily_note\|daily note]] | 0.36.0 |
| Escaped pipe | a \| b | 0.36.0 |

Move the cursor into the table to see it raw.

## Navigation and search

- **Alt+V** cycles the modes: source mode
  (every line raw Markdown, nothing folded or expanded; `mdedit -t` starts
  in it), then **view mode**: read-only, every line rendered (the cursor
  line too). In view mode ↑↓ move over rendered rows, **Tab** jumps to
  the next link (try it on the links in this note), **Enter** or a click
  follows it, and **Esc** goes back to editing.

- **Ctrl+V** pastes from the clipboard (copy something, then try it here);
  a URL pasted over a selection makes a link, as with the terminal's paste.

- **Page Up / Page Down** move a screen at a time.
- **Ctrl+F** searches: the cursor jumps to the first match as you type,
  and every match on screen is highlighted (the current one in red);
  ↑/↓ go to the previous/next match, Enter keeps the position, Esc goes
  back. Try searching for `needle`: here's one needle, and another NEEDLE.
- **F3 / Shift+F3** repeat the last search (next / previous).
- **Ctrl+H** finds and replaces (Ctrl+R too): type the search, Tab, type
  the replacement; Enter replaces the match and goes to the next one, ↑/↓
  skip one, Ctrl+A replaces all. Try replacing `kitten` with `cat`: one
  kitten, two kittens.
- Search is case-insensitive unless your search has a capital letter.
- **Ctrl+Z** undoes, up to 5 steps (typing goes back a word at a time;
  `undo_steps = 50` in `config.toml` keeps more), **Ctrl+Y** redoes.
  Undo everything you changed and the `[+]` unsaved mark goes away.
- Pasting inserts the text exactly as copied: a pasted list isn't
  continued or renumbered.
- **Shift + arrows** select text (Ctrl+A selects everything); selected lines
  are shown raw. With a selection, type `*` to wrap it in `*` … `*` (twice
  for bold), `==` to highlight, `[` twice for a wiki link, or paste a URL
  to turn it into a link. Try it on these words: make me bold.
- **Auto-pair**: typing `(`, `[`, `{` or a backtick adds the closer, and
  so does the second `*` of `**` (or `==`, `~~`). Type the closer yourself
  and it steps over; Backspace in an empty pair deletes both. Try typing
  `[[` here: 

***

Keys: Ctrl+O open · Ctrl+S save · Ctrl+Alt+S save as · Ctrl+X exit ·
Ctrl+F search · Ctrl+H replace · Ctrl+Z / Ctrl+Y undo / redo · Shift+arrows select · F3 / Shift+F3 next / previous · PgUp / PgDn ·
Ctrl+E emoji · Ctrl+T task · Ctrl+L close task · Ctrl+K fold ·
Ctrl+Enter / Alt+Enter follow link · Tab / Shift-Tab nest.
Settings in `~/.config/mdedit/config.toml`: `done_style = "grey"`,
`heading_size = "auto" | "on" | "off"`, `auto_pair = "off"`,
`undo_steps = 50`, `colors = "auto" | "truecolor" | "256" | "16"`,
`glyphs = "auto" | "unicode" | "ascii"`. mdedit detects what your terminal
supports; try `glyphs = "ascii"` or `colors = "16"` to see how it looks on
a basic terminal.
