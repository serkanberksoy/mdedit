# Technology & Skills

What mdedit is built with, which standards the code follows, and which
Claude Code skills and reference documents the project uses.

---

## 1. Technology stack

| Area | Technology | Version | Why |
|------|------------|---------|-----|
| Language | Rust | 1.94, edition 2024 | No garbage-collection pauses; the original POC requirements named Rust or Go |
| TUI framework | [ratatui](https://ratatui.rs) (crossterm backend) | 0.30 | The standard Rust TUI library; frame diffing and styled text |
| Terminal I/O | crossterm (re-exported by ratatui) | via ratatui | Raw mode, key events, cursor control |
| Text width | unicode-width | 0.2 | Column width of wide characters (emoji, CJK) for cursor placement |
| Graphemes | unicode-segmentation | 1.x | Cursor moves and deletes by grapheme cluster; soft wrap never splits one |
| Emoji data | emojis | 0.9 | Names, shortcodes and groups for the emoji picker (T-14), compiled in |
| Syntax highlighting | syntect (pure-Rust regex) | 5.3 | Colors fenced code blocks (B-09) |
| Terminal images | ratatui-image (no chafa) | 11.1 | Draws image embeds (E-04) with kitty, sixel, iTerm2 or half blocks; asks the terminal what it supports |
| Image decoding | image | 0.25 | PNG, JPEG, GIF, WebP, BMP for image embeds |
| Test runner (feature suite) | libtest-mimic | 0.8 | One named test per Markdown fixture, pending features shown as ignored |
| Terminal emulation (tests) | vt100 | 0.16 | Replays the real escape-sequence output to check what a terminal would show |
| Formatting | rustfmt (default style) | toolchain | Required by the Rust Style Guide |
| Linting | clippy, run with `-D warnings` | toolchain | Static verification (M-STATIC-VERIFICATION) |
| Real-terminal testing | Konsole + `qdbus` + `scripts/konsole_drive.py` | Konsole 26.08 | Catches terminal-specific rendering bugs the vt100 tests can't (see R-18) |

**Planned** (see `documentation/project_plan.md`):
- tree-sitter-markdown, an incremental parser for multi-line blocks (R-04, M1);
- syntect or tree-sitter highlighting for code blocks (B-09, M1);
- kitty/sixel graphics for images (E-04, M3).

### Architecture

```
src/
├── main.rs          terminal setup + event loop only (paste, key batching)
├── lib.rs           crate root (exposes the modules to tests)
├── cli.rs           command-line options
├── config.rs        ~/.config/mdedit/config.toml
├── shared.rs        what all editors share: settings, terminal, images, emoji, resolver
├── view.rs          EditorView: one document (text, undo, folds, prompts); keys → Outcome
├── resolver.rs      Resolver trait for links / embeds / images; RelativeResolver
├── processor.rs     CodeBlockProcessor trait: a host renders its own code blocks
├── app.rs           the mdedit program around one view: file browser, save prompt, exit
├── editor.rs        text buffer and grapheme-aware cursor (no rendering)
├── history.rs       undo / redo as line diffs (V-18)
├── selection.rs     text selection, wrapping it (V-19, V-05)
├── autopair.rs      auto-pair brackets and ** == ~~ (V-04)
├── images.rs        image embeds: links, sizes, loading, drawing (E-04, E-05)
├── search.rs        find, find and replace (V-16, V-17)
├── blocks.rs        document-level block scan (frontmatter, code fences,
│                    callouts, tables, folds), cached by text
├── markdown/
│   ├── mod.rs       line parser (Block), options
│   ├── inline.rs    emphasis, code spans, links, tags
│   ├── render.rs    rendered and raw line views
│   └── table.rs     table cells, alignment, borders
├── ui/
│   ├── mod.rs       EditorWidget (a view in any area), the program's screen, workarounds
│   ├── doc.rs       document layout: wrap, folds, tables, embeds, highlighting
│   └── popups.rs    dialogs and prompts
├── links.rs, embed.rs, highlight.rs, emoji.rs, files.rs, terminal.rs
└── wrap.rs          soft wrap with hanging indent, offset ↔ (row, x) mapping
tests/
├── features.rs      feature test runner (fixtures ↔ requirements/obsidian_features.md)
├── fixtures/        one .md per feature (+ .expected snapshot, optional .keys)
├── file_ops.rs      F-01 … F-07 against real files
├── embedding.rs     the embedding API, used like a host would
├── performance.rs   time budgets for large notes
├── robustness.rs    every note at many widths, fuzzing
└── project.rs       versions agree
```

### Terminal compatibility

mdedit detects what the terminal can do from its environment
(`src/terminal.rs`) and adapts. Everything also works in a plain
terminal; the extras are optional.

| Feature | How it adapts | Override (`config.toml`) |
|---------|---------------|--------------------------|
| Colors | 24-bit RGB if `COLORTERM=truecolor`; else the nearest 256-palette color (`TERM=*-256color`); else the nearest of the 16 basic colors | `colors = "auto" \| "truecolor" \| "256" \| "16"` |
| Symbols (box drawing, ☐ ☑, icons) | Unicode if the locale is UTF-8 and `TERM` isn't `linux`; otherwise ASCII look-alikes (`|` `+` `o` `x` …) and `?` for other characters | `glyphs = "auto" \| "unicode" \| "ascii"` |
| Emoji that draw too wide (flags, skin tones, joined) | Shown as a readable stand-in everywhere (R-18) | – |
| Larger headings | Off by default; DEC double-size lines only where known to work (Konsole, xterm, WezTerm) | `heading_size`, `--big-headings` |
| Ctrl+Shift+S, Ctrl+Enter | Kitty keyboard protocol when the terminal supports it; Ctrl+Alt+S and Alt+Enter work everywhere | – |
| Stale characters after redraws (seen in Konsole) | Every cell is repainted each frame (R-18) | – |

Tested for real in Konsole 26.08 (`tests/konsole.rs`); the generic
behavior is covered by the vt100 emulator test.

---

## 2. Coding standards

In order of precedence:

1. **[Rust Style Guide](https://doc.rust-lang.org/style-guide/)**, enforced by
   `cargo fmt`. Code is always formatted with default rustfmt settings.
2. **[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)**, the
   official library-team recommendations. Local copy of the checklist:
   `documentation/references/rust-api-guidelines/checklist.md`. Use it for
   anything `pub`.
3. **[Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/)**,
   practical rules with IDs (`M-…`). Local copy of the full text:
   `documentation/references/microsoft-pragmatic-rust-guidelines/pragmatic-rust-guidelines.md`.
4. **Clippy**, the default lint set with warnings treated as errors.
5. **`rust-skills` skill** (below): 265 detailed rules, consulted when
   writing or reviewing code.

### The rules we apply most

| Rule | Source | In mdedit |
|------|--------|-----------|
| Format with rustfmt, no manual style | Style Guide | `cargo fmt --check` is part of `scripts/check.sh` |
| No clippy warnings | M-STATIC-VERIFICATION | `cargo clippy --all-targets -- -D warnings` |
| Lint overrides use `#[expect(...)]` with a reason | M-LINT-OVERRIDE-EXPECT | Not `#[allow]` |
| Detected bugs are panics; expected failures are `Result` | M-PANIC-ON-BUG, M-PANIC-IS-STOP | e.g. file I/O returns errors; broken invariants panic |
| Panics have helpful messages | M-PANIC-MESSAGE | `expect("why this can't fail")` over `unwrap()` in production code |
| No `unsafe` | M-UNSAFE | There is none; adding any requires a written reason |
| Every module has module docs; first doc sentence ≈ 15 words | M-MODULE-DOCS, M-FIRST-DOC-SENTENCE | `//!` header on every file |
| Magic values are documented | M-DOCUMENTED-MAGIC | e.g. `TAB` width, colors per heading level |
| Integration tests live under `tests/` | M-INTEGRATION-TESTS | Feature suite in `tests/features.rs` |
| Tests don't just repeat the implementation (tautological) | M-TAUTOLOGICAL-TESTS | Snapshots are reviewed and written *before* the code (TDD), never blindly regenerated |
| I/O is mockable | M-MOCKABLE-SYSCALLS | Relevant for Save / Save As (F-01 to F-03): keep file access behind a small interface |
| Modules are balanced | M-BALANCED-MODULES | `markdown/` and `ui/` are split by concern (0.51.1) |
| Latest edition | M-LATEST-EDITION | Edition 2024 |

---

## 3. Claude Code skills

Installed per project under `.claude/skills/`, so every Claude Code session
in this repo loads them. Each folder has a `SOURCE.md` (origin, commit,
date) and the upstream `LICENSE`.

| Skill | Source | License | Use it for |
|-------|--------|---------|------------|
| `mdedit-feature-workflow` | this project | – | **Every feature and bugfix.** The TDD loop with fixtures and snapshots, then updating the status, VERSION.md and README |
| `test-driven-development` | [obra/superpowers](https://github.com/obra/superpowers) | MIT | The red-green-refactor discipline: no production code without a failing test first |
| `rust-testing` | [affaan-m/ECC](https://github.com/affaan-m/ECC) | MIT | Rust test patterns: unit/integration layout, `Result`-returning tests, proptest, coverage |
| `rust-skills` | [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills) | MIT | 265 idiomatic Rust rules (ownership, errors, API design, performance…) when writing or reviewing code |

Also available globally: `code-review` and `simplify` (review and cleanup
before finishing a feature).

### Reference documents (`documentation/references/`)

| Document | Source | License |
|----------|--------|---------|
| `microsoft-pragmatic-rust-guidelines/pragmatic-rust-guidelines.md` | [microsoft/rust-guidelines](https://github.com/microsoft/rust-guidelines) (agents `all.txt`, version 2026.6) | MIT |
| `rust-api-guidelines/checklist.md` | [rust-lang/api-guidelines](https://github.com/rust-lang/api-guidelines) | MIT OR Apache-2.0 |

These are unmodified copies. To refresh one, download it again and update
its `SOURCE.md`.

---

## 4. Testing skills & practices

| Level | Tool | Where | Catches |
|-------|------|-------|---------|
| Unit | `#[test]` in `#[cfg(test)]` modules | `src/*.rs` | Parser, editor, key-handling logic |
| Render | ratatui `TestBackend` | `src/ui/` | Screen layout, cursor placement, scrolling |
| Terminal output | vt100 emulator | `src/ui/` | Escape-sequence / frame-diff problems |
| **Feature** | libtest-mimic + Markdown fixtures + snapshots | `tests/features.rs`, `tests/fixtures/` | Each Obsidian feature, one fixture per feature, kept in sync with the requirements list |
| Project consistency | `#[test]` | `tests/project.rs` | Version in `Cargo.toml` = latest `VERSION.md` entry = README |
| File operations | real files in Cargo's temp dir | `tests/file_ops.rs` | Open / Save / Save As / Exit (F-01 to F-04), driven by keys |
| Real terminal | Konsole + pty + `qdbus` (`cargo test --test konsole -- --ignored`) | `tests/konsole.rs` | Konsole-specific wide-character bugs |

**Candidates for later** (recommended in the testing skills):
- `proptest` to fuzz the parser: rendering never panics, and the raw view is
  always the input unchanged;
- `cargo-nextest` for faster runs;
- `cargo-llvm-cov` for coverage, aiming for 80% or more.

---

## 5. Way of working

The binding rules are in `CLAUDE.md`. In short:
1. Every feature is built **test-first** (TDD).
2. Every feature has an **automated test**: at minimum its fixture snapshot
   in `tests/fixtures/`, plus unit tests.
3. After every feature: **bump the version**, and **update `VERSION.md` and
   `README.md`**.
