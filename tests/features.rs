//! Feature test suite: one Markdown fixture per feature in
//! `requirements/obsidian_features.md`, checked against an approved snapshot.
//!
//! See `tests/README.md` for the workflow. In short:
//!   cargo test --test features                      run all feature tests
//!   cargo test --test features 05-blocks            run one section
//!   UPDATE_SNAPSHOTS=1 cargo test --test features   (re)approve snapshots
//!                                                   and regenerate TEST_MATRIX.md

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use libtest_mimic::{Arguments, Failed, Trial};
use mdedit::app::App;
use mdedit::ui::render_app;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Style;
use ratatui::text::Line;

const FEATURES_DOC: &str = "requirements/obsidian_features.md";
const FIXTURES: &str = "tests/fixtures";
const MATRIX: &str = "tests/TEST_MATRIX.md";
/// Screen width that snapshots are rendered (and soft-wrapped) at.
const SNAPSHOT_WIDTH: usize = 80;
/// Screen height (text rows) for key scripts, e.g. Page Down moves this far.
const SNAPSHOT_HEIGHT: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Done,
    Partial,
    Open,
}

impl Status {
    fn icon(self) -> &'static str {
        match self {
            Status::Done => "✅",
            Status::Partial => "🟡",
            Status::Open => "⬜",
        }
    }
}

#[derive(Clone)]
struct Feature {
    id: String,
    name: String,
    milestone: String,
    status: Status,
    section: String,
}

#[derive(Clone)]
struct Fixture {
    /// e.g. `tests/fixtures/05-blocks/B-03-callout.md`
    md: PathBuf,
    keys: Option<PathBuf>,
    expected: PathBuf,
}

impl Fixture {
    fn rel(path: &Path) -> String {
        path.strip_prefix(root().join("tests"))
            .unwrap()
            .display()
            .to_string()
    }

    /// Test name, e.g. `05-blocks/B-03-callout`.
    fn name(&self) -> String {
        let dir = self
            .md
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy();
        let stem = self.md.file_stem().unwrap().to_string_lossy();
        format!("{dir}/{stem}")
    }
}

#[derive(PartialEq, Eq)]
enum Update {
    Off,
    /// Approve snapshots of done/partial features and refresh existing ones.
    Approved,
    /// Also write snapshots for open features (to hand-edit them into
    /// expectations before implementing).
    All,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn update_mode() -> Update {
    match std::env::var("UPDATE_SNAPSHOTS").as_deref() {
        Ok("all") => Update::All,
        Ok(v) if !v.is_empty() && v != "0" => Update::Approved,
        _ => Update::Off,
    }
}

// ---------------------------------------------------------------------------
// Feature list (the single source of truth for IDs, milestones and status)

/// Splits a Markdown table row on `|`, keeping escaped `\|` inside cells.
fn cells(row: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut prev = ' ';
    for c in row.chars() {
        if c == '|' && prev != '\\' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
        prev = c;
    }
    out.into_iter().map(|c| c.trim().to_string()).collect()
}

fn parse_features() -> Vec<Feature> {
    let doc = fs::read_to_string(root().join(FEATURES_DOC)).expect("features doc");
    let mut section = String::new();
    let mut out = Vec::new();
    for line in doc.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            section = h.to_string();
        }
        let c = cells(line);
        // | ID | Feature | Syntax | Now | Effort | Milestone | Notes |
        let is_row = c.len() >= 8
            && c[1].len() == 4
            && c[1].as_bytes()[1] == b'-'
            && c[1][2..].chars().all(|d| d.is_ascii_digit());
        if is_row {
            let status = if c[4].contains('✅') {
                Status::Done
            } else if c[4].contains('🟡') {
                Status::Partial
            } else {
                Status::Open
            };
            out.push(Feature {
                id: c[1].clone(),
                name: c[2].clone(),
                milestone: c[6].clone(),
                status,
                section: section.clone(),
            });
        }
    }
    out
}

fn discover_fixtures() -> BTreeMap<String, Vec<Fixture>> {
    let mut map: BTreeMap<String, Vec<Fixture>> = BTreeMap::new();
    let mut dirs: Vec<_> = fs::read_dir(root().join(FIXTURES))
        .unwrap()
        .flatten()
        .collect();
    dirs.sort_by_key(|d| d.path());
    for dir in dirs.into_iter().filter(|d| d.path().is_dir()) {
        let mut files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|f| f.path())
            .collect();
        files.sort();
        for md in files
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
        {
            let stem = md.file_stem().unwrap().to_string_lossy().to_string();
            let id = stem.get(..4).unwrap_or_default().to_string();
            let keys = md.with_extension("keys");
            map.entry(id).or_default().push(Fixture {
                keys: keys.exists().then_some(keys),
                expected: md.with_extension("expected"),
                md,
            });
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Key scripts: typed text plus `<Name>` keys; `//` lines are comments and
// newlines are ignored (use `<Enter>`).

/// One step of a key script: a key, or a paste (`<paste:text>`).
enum Input {
    Key(KeyEvent),
    Paste(String),
}

fn parse_keys(script: &str) -> Result<Vec<Input>, String> {
    let mut keys = Vec::new();
    let text: String = script
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("");
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '<' {
            keys.push(Input::Key(KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            )));
            continue;
        }
        let name: String = chars.by_ref().take_while(|&c| c != '>').collect();
        if let Some(text) = name.strip_prefix("paste:") {
            keys.push(Input::Paste(text.to_string()));
            continue;
        }
        let key = match name.as_str() {
            "S-Tab" => KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            n if n.starts_with("S-") => {
                let key = named_key(&n[2..])?;
                KeyEvent::new(key.code, key.modifiers | KeyModifiers::SHIFT)
            }
            n => named_key(n)?,
        };
        keys.push(Input::Key(key));
    }
    Ok(keys)
}

/// A key by its script name (`Enter`, `Left`, `C-x` …).
fn named_key(name: &str) -> Result<KeyEvent, String> {
    let none = KeyModifiers::NONE;
    Ok(match name {
        "Enter" => KeyEvent::new(KeyCode::Enter, none),
        "Tab" => KeyEvent::new(KeyCode::Tab, none),
        "BS" => KeyEvent::new(KeyCode::Backspace, none),
        "Del" => KeyEvent::new(KeyCode::Delete, none),
        "Up" => KeyEvent::new(KeyCode::Up, none),
        "Down" => KeyEvent::new(KeyCode::Down, none),
        "Left" => KeyEvent::new(KeyCode::Left, none),
        "Right" => KeyEvent::new(KeyCode::Right, none),
        "Home" => KeyEvent::new(KeyCode::Home, none),
        "End" => KeyEvent::new(KeyCode::End, none),
        "PgUp" => KeyEvent::new(KeyCode::PageUp, none),
        "PgDn" => KeyEvent::new(KeyCode::PageDown, none),
        "F3" => KeyEvent::new(KeyCode::F(3), none),
        "Esc" => KeyEvent::new(KeyCode::Esc, none),
        "lt" => KeyEvent::new(KeyCode::Char('<'), none),
        n if n.starts_with("C-") && n.chars().count() == 3 => KeyEvent::new(
            KeyCode::Char(n.chars().nth(2).expect("counted")),
            KeyModifiers::CONTROL,
        ),
        n if n.starts_with("A-") && n.chars().count() == 3 => KeyEvent::new(
            KeyCode::Char(n.chars().nth(2).expect("counted")),
            KeyModifiers::ALT,
        ),
        other => return Err(format!("unknown key <{other}>")),
    })
}

// ---------------------------------------------------------------------------
// Snapshots: rendered lines with styles written inline as «text»{style}.

fn style_desc(s: Style) -> String {
    let mut parts = Vec::new();
    if let Some(fg) = s.fg {
        parts.push(format!("{fg:?}").to_lowercase().replace(' ', ""));
    }
    if let Some(bg) = s.bg {
        parts.push(format!(
            "bg:{}",
            format!("{bg:?}").to_lowercase().replace(' ', "")
        ));
    }
    if !s.add_modifier.is_empty() {
        parts.extend(
            format!("{:?}", s.add_modifier)
                .to_lowercase()
                .split(" | ")
                .map(str::to_string),
        );
    }
    parts.join(" ")
}

fn render_line_markup(line: &Line) -> String {
    // Merge neighbouring spans with the same style.
    let mut merged: Vec<(Style, String)> = Vec::new();
    for span in line.spans.iter().filter(|s| !s.content.is_empty()) {
        let style = line.style.patch(span.style);
        match merged.last_mut() {
            Some((s, text)) if *s == style => text.push_str(&span.content),
            _ => merged.push((style, span.content.to_string())),
        }
    }
    merged
        .into_iter()
        .map(|(style, text)| {
            let desc = style_desc(style);
            if desc.is_empty() {
                text
            } else {
                format!("«{text}»{{{desc}}}")
            }
        })
        .collect()
}

fn snapshot(fixture: &Fixture) -> Result<String, String> {
    let md = fs::read_to_string(&fixture.md).map_err(|e| e.to_string())?;
    let mut app = App::new(&md, None);
    app.view.width = SNAPSHOT_WIDTH;
    app.view.height = SNAPSHOT_HEIGHT;
    let mut out = String::new();
    let cursor = match &fixture.keys {
        Some(keys) => {
            let script = fs::read_to_string(keys).map_err(|e| e.to_string())?;
            for input in parse_keys(&script)? {
                match input {
                    Input::Key(key) => {
                        app.handle_key(key);
                    }
                    Input::Paste(text) => app.handle_paste(&text),
                }
            }
            let _ = writeln!(out, "--- source after keys ---");
            out.push_str(&app.view.editor.to_text());
            let _ = writeln!(
                out,
                "--- rendered (cursor on line {}) ---",
                app.view.editor.row + 1
            );
            Some(app.view.editor.row)
        }
        None => {
            let _ = writeln!(out, "--- rendered ---");
            None
        }
    };
    let base = fixture.md.parent();
    for line in render_app(&app, cursor, SNAPSHOT_WIDTH, base) {
        let _ = writeln!(out, "{}", render_line_markup(&line));
    }
    Ok(out)
}

fn diff(expected: &str, actual: &str) -> String {
    let (e, a): (Vec<_>, Vec<_>) = (expected.lines().collect(), actual.lines().collect());
    let mut out = String::new();
    for i in 0..e.len().max(a.len()) {
        let (el, al) = (e.get(i).copied(), a.get(i).copied());
        if el == al {
            let _ = writeln!(out, "    {}", el.unwrap_or_default());
        } else {
            if let Some(el) = el {
                let _ = writeln!(out, "  - {el}");
            }
            if let Some(al) = al {
                let _ = writeln!(out, "  + {al}");
            }
        }
    }
    out
}

fn will_have_snapshot(f: &Feature, fx: &Fixture, update: &Update) -> bool {
    fx.expected.exists()
        || match update {
            Update::Off => false,
            Update::Approved => f.status != Status::Open,
            Update::All => true,
        }
}

fn run_fixture(f: &Feature, fx: &Fixture, update: &Update) -> Result<(), Failed> {
    let actual = snapshot(fx)?;
    if *update != Update::Off && will_have_snapshot(f, fx, update) {
        fs::write(&fx.expected, &actual)?;
        return Ok(());
    }
    match fs::read_to_string(&fx.expected) {
        Ok(expected) if expected == actual => Ok(()),
        Ok(expected) => Err(format!(
            "{} {} {}: rendering differs from {}\n(- expected, + actual)\n{}",
            f.status.icon(),
            f.id,
            f.name,
            Fixture::rel(&fx.expected),
            diff(&expected, &actual)
        )
        .into()),
        Err(_) => Err(format!(
            "{} {} is marked {} in {FEATURES_DOC} but has no approved snapshot.\n\
             Review the output below, then run: UPDATE_SNAPSHOTS=1 cargo test --test features\n{actual}",
            f.id,
            f.name,
            f.status.icon()
        )
        .into()),
    }
}

// ---------------------------------------------------------------------------
// Consistency checks and the generated matrix

fn check_consistency(
    features: &[Feature],
    fixtures: &BTreeMap<String, Vec<Fixture>>,
) -> Result<(), Failed> {
    let mut problems = Vec::new();
    for f in features {
        match fixtures.get(&f.id).map(Vec::len) {
            None => problems.push(format!("{} {}: no fixture .md file", f.id, f.name)),
            Some(1) => {}
            Some(n) => problems.push(format!("{}: {n} fixture files, expected exactly 1", f.id)),
        }
    }
    for (id, list) in fixtures {
        if !features.iter().any(|f| &f.id == id) {
            for fx in list {
                problems.push(format!(
                    "{}: no row {id} in {FEATURES_DOC}",
                    Fixture::rel(&fx.md)
                ));
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n").into())
    }
}

fn matrix(
    features: &[Feature],
    fixtures: &BTreeMap<String, Vec<Fixture>>,
    update: &Update,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Feature Test Matrix\n");
    let _ = writeln!(
        out,
        "Generated by `tests/features.rs` from `{FEATURES_DOC}` and `tests/fixtures/`.\n\
         Do not edit by hand. Regenerate with `UPDATE_SNAPSHOTS=1 cargo test --test features`.\n"
    );
    let _ = writeln!(
        out,
        "**Test** column: ✅ checked against its approved snapshot · ⏸ pending (feature not built yet) · ❌ missing snapshot\n"
    );

    // Summary per milestone.
    let mut summary: BTreeMap<String, [usize; 4]> = BTreeMap::new();
    for f in features {
        let fx = fixtures.get(&f.id).and_then(|l| l.first());
        let s = summary.entry(f.milestone.clone()).or_default();
        s[0] += 1;
        match fx.map(|fx| will_have_snapshot(f, fx, update)) {
            Some(true) => s[1] += 1,
            _ if f.status == Status::Open => s[2] += 1,
            _ => s[3] += 1,
        }
    }
    let _ = writeln!(out, "## Summary\n");
    let _ = writeln!(
        out,
        "| Milestone | Features | ✅ Checked | ⏸ Pending | ❌ Missing |"
    );
    let _ = writeln!(
        out,
        "|-----------|---------:|-----------:|----------:|-----------:|"
    );
    for (m, s) in &summary {
        let _ = writeln!(out, "| {m} | {} | {} | {} | {} |", s[0], s[1], s[2], s[3]);
    }

    let mut section = "";
    for f in features {
        if f.section != section {
            section = &f.section;
            let _ = writeln!(out, "\n## {section}\n");
            let _ = writeln!(
                out,
                "| ID | Feature | Milestone | Now | Fixture | Keys | Snapshot | Test |"
            );
            let _ = writeln!(
                out,
                "|----|---------|-----------|-----|---------|------|----------|------|"
            );
        }
        let fx = fixtures.get(&f.id).and_then(|l| l.first());
        let link = |p: &Path, label: &str| format!("[{label}]({})", Fixture::rel(p));
        let (md, keys, snap, test): (String, String, String, String) = match fx {
            None => ("—".into(), "—".into(), "—".into(), "❌ no fixture".into()),
            Some(fx) => {
                let has = will_have_snapshot(f, fx, update);
                let file = fx.md.file_name().unwrap().to_string_lossy().to_string();
                (
                    link(&fx.md, &file),
                    fx.keys.as_deref().map_or("—".into(), |k| link(k, "keys")),
                    if has {
                        link(&fx.expected, "expected")
                    } else {
                        "—".into()
                    },
                    if has {
                        "✅ checked".into()
                    } else if f.status == Status::Open {
                        "⏸ pending".into()
                    } else {
                        "❌ missing".into()
                    },
                )
            }
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {md} | {keys} | {snap} | {test} |",
            f.id,
            f.name,
            f.milestone,
            f.status.icon()
        );
    }
    out
}

fn main() {
    let args = Arguments::from_args();
    let update = update_mode();
    let features = parse_features();
    let fixtures = discover_fixtures();

    let mut trials = Vec::new();
    {
        let (features, fixtures) = (features.clone(), fixtures.clone());
        trials.push(Trial::test(
            "00-suite/every-feature-has-one-fixture",
            move || check_consistency(&features, &fixtures),
        ));
    }
    {
        let text = matrix(&features, &fixtures, &update);
        let write = update != Update::Off;
        trials.push(Trial::test("00-suite/test-matrix-up-to-date", move || {
            let path = root().join(MATRIX);
            if write {
                fs::write(&path, &text)?;
                return Ok(());
            }
            match fs::read_to_string(&path) {
                Ok(current) if current == text => Ok(()),
                _ => Err(format!(
                    "{MATRIX} is out of date. Regenerate: UPDATE_SNAPSHOTS=1 cargo test --test features"
                )
                .into()),
            }
        }));
    }

    for f in &features {
        let Some(fx) = fixtures.get(&f.id).and_then(|l| l.first()).cloned() else {
            continue;
        };
        let pending = f.status == Status::Open && !will_have_snapshot(f, &fx, &update);
        let (kind, f, update) = (f.milestone.clone(), f.clone(), update_mode());
        trials.push(
            Trial::test(fx.name(), move || run_fixture(&f, &fx, &update))
                .with_kind(kind)
                .with_ignored_flag(pending),
        );
    }

    libtest_mimic::run(&args, trials).exit();
}
