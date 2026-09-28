# Emoji Picker Requirements (T-14)

Ctrl+E opens an emoji picker: search by name, move through the results with
the arrow keys, and Enter inserts the emoji at the cursor. Milestone 1, text
formatting; listed as **T-14** in `requirements/obsidian_features.md`.

Reference: the Obsidian plugin
[oliveryh/obsidian-emoji-toolbar](https://github.com/oliveryh/obsidian-emoji-toolbar),
used in our notes (e.g. "the emoji toolbar 🌱" in
`example_mds/daily_note.md`). It's built on Emoji Mart.

Status: EP-01 … EP-05 ✅ (0.10.0, Milestone 1) · EP-06 ✅ (0.11.0, done early) · EP-07 … EP-12 in **Milestone 4**

---

## Requested behavior

- **EP-01 Ctrl+E opens the picker** ✅ P1
  A popup window over the editor shows a grid of emoji, with the focus in a
  **search field** at the top. The editor text isn't changed.

- **EP-02 Search** ✅ P1
  Typing filters the emoji by their text: name (e.g. `grinning face`) and
  shortcodes (e.g. `smile`, `+1`, `thumbsup`). **Limitation:** there are no
  keyword lists (e.g. `happy`), because the `emojis` crate doesn't have them.
  Adding CLDR annotations would fill this gap (M4, with EP-12). Backspace edits the search. With an empty
  search, all emoji are shown (see EP-06 for the order).

- **EP-03 Arrow keys move through the results** ✅ P1
  After searching, the arrow keys move the highlight through the results
  (Left/Right within a row, Up/Down between rows of the grid). The grid
  scrolls to keep the highlight visible. Typing more text returns to the
  search and re-filters, with the highlight back on the first result.

- **EP-04 Enter inserts and closes** ✅ P1
  Enter inserts the highlighted emoji **at the cursor** (the cursor moves
  after it, and the document is marked unsaved), and the picker closes.
  Enter with no results does nothing. **Esc**, or **Ctrl+E** again, closes
  the picker without inserting anything.

## Found in the Obsidian plugin

The Obsidian emoji toolbar plugin also has these. EP-05 is part of T-14 in
Milestone 1; **EP-06 to EP-12 are in Milestone 4**.

- **EP-05 Fuzzy search** ✅ P2 (Milestone 1, part of EP-02)
  Near-misses match, not just exact names. Ranking, best first: exact name
  or shortcode → name/shortcode prefix → start of a word (`up` finds 👍
  "thumbs up") → anywhere in the text → fuzzy, the letters in order
  (`grnng fc` finds 😀). Ties keep Unicode order.

- **EP-06 Recently used** ✅ P2 · M4, done early in 0.11.0
  The emoji inserted most recently appear first when the search is empty.
  They're remembered between sessions in a small file in the user's config
  folder (e.g. `~/.config/mdedit/recent_emoji`).
  ✅ Up to 32 emoji (two grid rows), most recent first, no duplicates, in
  `$XDG_CONFIG_HOME/mdedit/recent_emoji` (default `~/.config/…`), one per
  line. Unknown lines are skipped when loading. A typed search ranks as
  usual; recents only lead the empty search. If the file can't be written,
  the emoji is still inserted and the status bar says so.

- **EP-07 Skin tones** ⬜ P3 · M4
  A skin-tone setting (default, light … dark) is applied to emoji that
  support it. The plugin keeps it as a setting; in mdedit it could be a key
  in the picker (e.g. Tab cycles the tone), remembered like EP-06.
  **Terminal risk:** skin-tone emoji are two code points (emoji + modifier).
  Some terminals draw them as two glyphs. **Correction (0.20.0):** a
  cursor-position query in Konsole reports 👍🏽 as 2 wide, but Konsole
  *draws* it wider and swallows the text after it (`tests/konsole.rs`). mdedit
  now shows it as 👍 (modifier dropped, display only). So in Konsole a skin
  tone could be inserted but wouldn't be visible. Decide in M4 whether the
  picker should offer tones at all.

- **EP-08 Categories** ⬜ P3 · M4
  Emoji Mart groups emoji into categories (Smileys & People, Animals &
  Nature, Food & Drink, …) with a tab bar. In the terminal: show category
  headers in the empty-search grid, with keys to jump between categories
  (e.g. PageUp/PageDown).

- **EP-09 Preview line** ⬜ P2 · M4
  The bottom of the picker shows the highlighted emoji large, plus its name
  and shortcode (e.g. `😀 grinning face  :grinning:`), so you know what
  you're inserting.

- **EP-10 Twitter emoji (Twemoji) images** ✗ not applicable
  The plugin can show Twitter's emoji artwork. A terminal draws emoji with
  its own font, so this doesn't apply. (Terminal graphics could draw it in
  M3 with E-04, but that's not worth it for a picker.)

- **EP-11 Mouse** ⬜ P3 · M4
  The plugin inserts on click. mdedit is keyboard-first; mouse support would
  come with general mouse handling, if ever.

## Related, not in the plugin

- **EP-12 `:shortcode:` autocomplete** ⬜ P3 · M4
  Typing `:smi` in the text suggests matching emoji inline (as the separate
  Obsidian "Emoji Shortcodes" plugin does). Candidate for later; it can
  reuse EP-02/EP-05 search.

---

## Implementation notes

- **Emoji data:** the [`emojis`](https://crates.io/crates/emojis) crate has
  names, shortcodes, groups (categories) and skin-tone variants, compiled in
  with no runtime files. Keywords would need extra data (e.g. CLDR
  annotations) if name and shortcode search isn't enough.
- **Grid layout:** each emoji is 2 columns wide. Cells are 3 columns (emoji
  + space), so a 60-column popup shows 20 emoji per row.
- **Emoji left out of the picker** (`emoji::all`): joined emoji (ZWJ,
  e.g. 🧑‍🦳) and flags (regional-indicator pairs like 🇹🇷, and tag sequences
  like 🏴󠁧󠁢󠁥󠁮󠁧󠁿). Konsole draws them wider than the 2 columns they're
  measured as, which broke the grid. `tests/konsole.rs` found the flags.
  Single-glyph flags (🏁 🚩 🏳️) stay. Emoji with a variation selector (⬆️ ❤️)
  are fine: Konsole draws them 2 wide, although its text dump leaves the
  selector out. **Limitation:** flags can't be inserted from the picker yet.
  They'd need the same display workaround as ZWJ sequences (M4).
- **Tests:**
  - feature fixture `tests/fixtures/01-text-formatting/T-14-emoji-picker.md`
    with a `.keys` script (`<C-e>grinning<Enter>`);
  - unit tests for search ranking;
  - a UI test for the popup.
