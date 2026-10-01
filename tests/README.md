# mdedit test suite

There are six kinds of tests:

| Suite | Where | What it covers |
|-------|-------|----------------|
| **Unit tests** | `src/**` (`#[cfg(test)]` modules) | Parser, editor operations, key handling, screen layout, terminal output (vt100) |
| **Feature tests** | `tests/features.rs` + `tests/fixtures/` | One Markdown fixture per feature in `requirements/obsidian_features.md`, each checked against an approved snapshot |
| **Project checks** | `tests/project.rs` | The version in `Cargo.toml`, `VERSION.md` and `README.md` agree |
| **Robustness** | `tests/robustness.rs` | No note may crash the renderer: every example note and fixture at many widths with the cursor on every line, plus 10,000 random strings of markup and multi-byte text |
| **File operations** | `tests/file_ops.rs` | Open / Save / Save As / Exit (F-01 to F-04) on real files in Cargo's temp dir |
| **Real terminal** | `tests/konsole.rs` (ignored by default) | Runs the binary in Konsole and compares its screen with ratatui's drawing; `cargo test --test konsole -- --ignored` |

```bash
cargo test                                  # everything
cargo test --test features                  # feature tests only
cargo test --test features 05-blocks        # one section
cargo test --test features B-03             # one feature
cargo test --test features -- --ignored     # run the pending (not built yet) features
```

## Feature tests

### Layout

```
tests/
├── features.rs                 test runner (one test per fixture)
├── TEST_MATRIX.md              generated: feature ↔ fixture ↔ snapshot ↔ status
└── fixtures/
    ├── 01-text-formatting/
    │   ├── T-01-atx-headings.md        input Markdown
    │   └── T-01-atx-headings.expected  approved rendering
    ├── …
    └── 09-editing-behaviors/
        ├── V-02-list-continuation.md
        ├── V-02-list-continuation.keys      keys typed before the snapshot
        └── V-02-list-continuation.expected
```

- **Every feature row has exactly one fixture.** The file name starts with
  the feature ID (`B-03-callout.md`). The `00-suite/every-feature-has-one-fixture`
  test enforces this in both directions.
- **Status comes from the feature list.** The **Now** column of
  `requirements/obsidian_features.md` decides how a fixture is treated:

  | Now | Snapshot exists | Result |
  |-----|-----------------|--------|
  | ✅ / 🟡 | yes | must match |
  | ✅ / 🟡 | no | **fails** ("no approved snapshot") |
  | ⬜ | no | ignored (⏸ pending) |
  | ⬜ | yes | must match (the expectation was written first, TDD) |

- **`TEST_MATRIX.md`** is regenerated from the above. The
  `00-suite/test-matrix-up-to-date` test fails if it's stale.

### Snapshot format (`.expected`)

Each fixture line is rendered as the editor shows it away from the cursor.
Styled text is written `«text»{style}`, for example `«▌ Heading 2»{cyan bold}`
or `«done»{darkgray crossed_out}`. Unstyled text is written plainly.

With a `.keys` file, the snapshot has two parts: the source after the keys
were typed, and the rendering with the cursor where the keys left it (that
line shows as raw Markdown).

### Key scripts (`.keys`)

Plain characters are typed as-is. Special keys: `<Enter> <Tab> <S-Tab> <BS>
<Del> <Up> <Down> <Left> <Right> <Home> <End> <PgUp> <PgDn> <F3> <S-F3>
<Esc> <lt>` (a literal `<`), `<C-x>` for Ctrl+x, `<A-x>` for Alt+x, `<S-…>` for a key with
Shift (`<S-Left>`, `<S-F3>`), and `<paste:text>` for pasted text. Key scripts run with a
screen of 80 × 10 text rows. Newlines are ignored, and lines starting with `//` are
comments.

### Workflow for a new feature (TDD)

1. **Write the expectation first:** edit or create
   `tests/fixtures/<section>/<ID>-<slug>.expected` by hand. For a starting
   point, run `UPDATE_SNAPSHOTS=all cargo test --test features <ID>` to get
   the current (wrong) output, then edit it to what it should be.
2. **Red:** `cargo test --test features <ID>` fails and shows a diff.
3. **Green:** implement the feature (plus unit tests in `src/`) until it
   passes.
4. **Update the status:** set **Now** to ✅ (or 🟡) in
   `requirements/obsidian_features.md`.
5. **Regenerate the matrix:** `UPDATE_SNAPSHOTS=1 cargo test --test features`.
6. **Release:** bump the version in `Cargo.toml` (MINOR for a feature,
   PATCH for a fix) and update `VERSION.md` and `README.md`.
   `tests/project.rs` checks that they agree.
7. `scripts/check.sh` must pass (format, lints, all tests).

The full checklist is the `mdedit-feature-workflow` skill
(`.claude/skills/mdedit-feature-workflow/SKILL.md`).

`UPDATE_SNAPSHOTS=1` overwrites snapshots with the current output. Always
review the diff (`git diff tests/fixtures`) before committing. An approved
snapshot is a claim that the output is *correct*, not just current.
