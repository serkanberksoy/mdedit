# mdedit: instructions for Claude Code

A terminal Markdown editor with Obsidian-style Live Preview, written in Rust
with ratatui. Start with `README.md`. The plan is in
`documentation/project_plan.md`, and the stack and standards are in
`documentation/technology_and_skills.md`.

## Scope

mdedit edits one Markdown file at a time and must **never** depend on a
vault (an index of other notes). Vault features are for a separate wrapper
project (milestone **Wrapper** in `requirements/obsidian_features.md`; see
the plan, section 6). Links resolve relative to the current file.

mdedit is also a library for such hosts (`documentation/embedding.md`):
`Shared` + one `EditorView` per document + `ui::EditorWidget` + the
`Resolver` trait. Keep host code (tabs, file lists, vaults, demo hosts) out
of this repository; the `mdedit` binary (`app.rs`, `main.rs`) is the only
host here, and it must keep working on its own.

## Way of working (mandatory)

1. **Every feature is implemented with TDD.** Write the failing test first
   (fixture snapshot and/or unit test), watch it fail for the right reason,
   then implement, then refactor. Use the `mdedit-feature-workflow` skill
   for every feature or bugfix; it drives `test-driven-development`.
2. **Every feature is automatically tested.** Each feature row in
   `requirements/obsidian_features.md` has exactly one fixture in
   `tests/fixtures/`, checked by `cargo test`. Editing behaviors use
   `.keys` scripts. Other requirements (F-xx, R-xx) get unit or integration
   tests.
3. **After every implemented feature:** bump the version in `Cargo.toml`
   (MINOR for a feature, PATCH for a fix), add the `VERSION.md` entry at
   the top, update `README.md`, and **update
   `example_mds/feature_showcase.md`**: demonstrate the new feature there
   and set its frontmatter `version:` to the new version.
   `tests/project.rs` enforces that the versions agree.

A feature is **done** only when `scripts/check.sh` passes (rustfmt,
`clippy -D warnings`, all tests) and the status in the requirements docs
and the project plan is updated.

## Commands

`cargo` is in `~/.cargo/bin` and may not be on `PATH`. Use `export
PATH=$HOME/.cargo/bin:$PATH` or the full path.

```bash
scripts/check.sh                                  # the gate
cargo test --test features <ID or section>        # feature tests
UPDATE_SNAPSHOTS=1 cargo test --test features     # approve snapshots + regenerate tests/TEST_MATRIX.md
UPDATE_SNAPSHOTS=all cargo test --test features <ID>   # starting point for a new expectation
cargo test --test konsole -- --ignored            # real-terminal check (opens a Konsole window)
cargo run -- example_mds/daily_note.md            # try it (needs a real terminal)
```

## Code standards

- Default rustfmt; no clippy warnings; no `unsafe`; `#[expect(lint, reason = "…")]`
  instead of `#[allow]`.
- Expected failures (I/O) → `Result`; broken invariants → panic with a
  message.
- Every module starts with `//!` docs. Match the surrounding code's comment
  density and naming.
- Skills: `rust-skills` (idiomatic rules), `rust-testing` (test patterns).
  References are in `documentation/references/`.

## Gotchas

- Rendering is display-only: never change the buffer text to fix how
  something looks (e.g. joined emoji are replaced only in the rendered
  frame, in `ui::apply_terminal_workarounds`).
- The feature snapshots use `ui::render_app` (and `render_document`);
  `EditorWidget` draws the same rows. Both go through `ui/doc.rs` (`Doc`),
  so keep layout changes there.
- Konsole disagrees with ratatui about wide characters; see R-18 in
  `requirements/obsidian_parity_requirements.md` before touching cell
  diffing.
- A snapshot approved with `UPDATE_SNAPSHOTS=1` is a claim of *correct*
  output. Review the diff; never approve blindly.
