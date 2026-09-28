---
name: mdedit-feature-workflow
description: The mandatory workflow for implementing any feature or bugfix in mdedit. Use it whenever you implement, change or fix editor behavior, or pick up an item from requirements/obsidian_features.md, file_handling_requirements.md or obsidian_parity_requirements.md. Covers the TDD loop with Markdown fixtures and snapshots, updating the feature status, the version bump, and the VERSION.md/README update.
---

# mdedit feature workflow

Every feature is **test-first**, **automatically tested**, and **released**
with a version bump. No exceptions without the user's explicit permission.

Related skills: `test-driven-development` (the discipline), `rust-testing`
(test patterns), `rust-skills` (code rules).

## 0. Pick the item

- Find its row: `requirements/obsidian_features.md` (IDs like `T-06`,
  `B-11`), or `F-xx` / `R-xx` in the other requirement files.
- Check the milestone in `documentation/project_plan.md`. Work on the
  current milestone unless the user says otherwise.
- Read the fixture: `tests/fixtures/<section>/<ID>-<slug>.md` (and `.keys`
  if there is one). Extend it if it doesn't cover the edge cases.

## 1. RED: write the expectation first

**Rendering features:** write
`tests/fixtures/<section>/<ID>-<slug>.expected` by hand.
- To get a starting point: `UPDATE_SNAPSHOTS=all cargo test --test features <ID>`.
  This writes the *current* (wrong) output. Then edit it into the
  *correct* output.
- Format: `«text»{style}` for styled runs (e.g. `«struck»{crossed_out}`).
  See `tests/README.md`.

**Editing behaviors:** add or extend `<ID>-<slug>.keys`
(`<Enter> <Tab> <S-Tab> <C-t> …`). The snapshot includes the source after
the keys.

**Logic:** add unit tests in the module's `#[cfg(test)] mod tests` (parser
in `markdown/`, buffer operations in `editor.rs`, keys in `app.rs`, layout
in `ui/`).

**Features without a fixture row** (e.g. F-01 to F-03, file handling):
write unit or integration tests (for file I/O, use a temp dir under
`target/`, never the user's files).

Then **watch it fail for the right reason:**
```bash
cargo test --test features <ID>      # must FAIL with the diff you expect
cargo test <unit_test_name>          # must FAIL (not a compile error you didn't intend)
```
If it passes already, the test is wrong or the feature already exists. Stop
and re-check.

## 2. GREEN: minimal implementation

Write the least code that makes the tests pass. Follow
`documentation/technology_and_skills.md` §2: rustfmt, no clippy warnings,
no `unsafe`, errors as `Result`, bugs as panics with messages, module docs.

```bash
cargo test --test features <ID>      # now passes
cargo test                           # EVERYTHING passes, not just your test
```

## 3. REFACTOR

Clean up with the tests green. Keep modules balanced (split a module
when it grows too big, like `markdown/` and `ui/`).

## 4. Update the status

- Set **Now** to ✅ (fully done) or 🟡 (partial, and say what's missing in
  Notes) in `requirements/obsidian_features.md`. For R-/F- items, update
  the status in their file.
- Regenerate the snapshots and matrix, then **review the diff**:
  ```bash
  UPDATE_SNAPSHOTS=1 cargo test --test features
  ```
  An ✅/🟡 feature without an approved snapshot fails the suite. A
  snapshot is a claim of *correct* output; never approve output you
  haven't read.

## 5. Release: version, VERSION.md, README

1. `Cargo.toml` `version`: bump **MINOR** for a feature, **PATCH** for a
   fix, refactor, docs or tests only (rules in `VERSION.md`).
2. `VERSION.md`: new entry at the **top**, `## X.Y.Z (YYYY-MM-DD)` with
   *Added / Changed / Fixed* naming the feature IDs, and update
   `Current version: **X.Y.Z**`.
3. `README.md`: update `**Version:** X.Y.Z`, the feature count ("N of 88
   tracked features"), and the Features table or key table if the change
   is user-visible.
4. `documentation/project_plan.md`: update the status icons for the item.
5. `example_mds/feature_showcase.md`: add a demonstration of the new
   feature (in the right section, with a line saying how to try it) and
   set its frontmatter `version:` to the new version. This is the note
   used to try every feature by hand.

`scripts/release.py X.Y.Z entry.md` does steps 1–3 and the showcase
`version:` (write the *Added / Changed / Fixed* text to `entry.md` first);
the showcase demonstration, the README tables and the plan are still yours.

`tests/project.rs` fails if Cargo.toml, VERSION.md, the README and the
showcase disagree about the version.



## 6. Gate

```bash
scripts/check.sh    # fmt --check, clippy -D warnings, cargo test
```
It must end with `OK`. Report the result to the user, including any
failure, by name.

## Batching

Several small features in one session: do steps 1–4 **per feature**, one
version bump per feature (0.3.0, 0.4.0, …), each with its own VERSION.md
entry.
