//! Project consistency checks: the version in `Cargo.toml` must match the
//! latest `VERSION.md` entry and the README, so a feature can't ship without
//! its version bump and docs update.

use std::fs;
use std::path::Path;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version entries in VERSION.md: `## X.Y.Z (date)` headings, newest first.
fn version_headings(history: &str) -> Vec<&str> {
    history
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .filter(|h| h.starts_with(|c: char| c.is_ascii_digit()))
        .map(|h| h.split_whitespace().next().unwrap_or_default())
        .collect()
}

fn read(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {file}: {e}"))
}

#[test]
fn version_md_latest_entry_matches_cargo_version() {
    let history = read("VERSION.md");
    let latest = *version_headings(&history)
        .first()
        .expect("VERSION.md has no `## X.Y.Z` entry");
    assert_eq!(
        latest, VERSION,
        "the newest VERSION.md entry must be the Cargo.toml version"
    );
}

#[test]
fn version_md_current_version_matches_cargo_version() {
    let expected = format!("Current version: **{VERSION}**");
    assert!(
        read("VERSION.md").contains(&expected),
        "VERSION.md must say `{expected}`"
    );
}

#[test]
fn readme_shows_cargo_version() {
    let expected = format!("**Version:** {VERSION}");
    assert!(
        read("README.md").contains(&expected),
        "README.md must say `{expected}`"
    );
}

#[test]
fn version_entries_are_newest_first_and_unique() {
    let history = read("VERSION.md");
    let versions: Vec<Vec<u64>> = version_headings(&history)
        .into_iter()
        .map(|v| {
            v.split('.')
                .map(|n| n.parse().expect("version parts are numbers"))
                .collect()
        })
        .collect();
    assert!(
        versions.windows(2).all(|w| w[0] > w[1]),
        "VERSION.md entries must be strictly newest first: {versions:?}"
    );
}

#[test]
fn feature_showcase_is_updated_with_every_release() {
    // example_mds/feature_showcase.md demonstrates every implemented feature;
    // its frontmatter version is bumped whenever it's updated for a release.
    let expected = format!("version: {VERSION}");
    assert!(
        read("example_mds/feature_showcase.md").contains(&expected),
        "update example_mds/feature_showcase.md for this release (new features) and set `{expected}`"
    );
}

/// `mdedit --help`, from the real binary.
fn help() -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_mdedit"))
        .arg("--help")
        .output()
        .expect("mdedit --help runs");
    assert!(out.status.success());
    String::from_utf8(out.stdout).expect("UTF-8 help")
}

/// The part of `text` from the line starting with `from` up to the line
/// starting with `to`.
fn section<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let start = text.find(from).unwrap_or_else(|| panic!("no {from:?}"));
    let end = text[start..].find(to).map_or(text.len(), |e| start + e);
    &text[start..end]
}

/// Key names like `Ctrl+Shift+Z`, `Alt+Enter` or `F3` in `text`.
fn key_names(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for (i, _) in text.match_indices(['C', 'A', 'F']) {
        let rest = &text[i..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '+')
            .collect();
        let name = name.trim_end_matches('+');
        let is_key = ["Ctrl+", "Alt+"].iter().any(|p| name.starts_with(p))
            || (name.len() >= 2
                && name.starts_with('F')
                && name[1..].chars().all(|c| c.is_ascii_digit()));
        let starts_word =
            i == 0 || !text[..i].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '+');
        if is_key && starts_word {
            keys.push(name.to_string());
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

#[test]
fn help_lists_every_key_in_the_readme() {
    let readme = read("README.md");
    let keys = key_names(section(&readme, "| Key | Action |", "## Settings"));
    assert!(keys.len() > 15, "found keys: {keys:?}");
    let help = help();
    let missing: Vec<_> = keys.iter().filter(|k| !help.contains(k.as_str())).collect();
    assert!(missing.is_empty(), "keys missing from --help: {missing:?}");
}

#[test]
fn help_lists_every_setting_and_option_in_the_readme() {
    let readme = read("README.md");
    let help = help();
    let settings = section(&readme, "## Settings", "## Development");
    let names: Vec<&str> = settings
        .lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim()))
        .filter(|k| !k.starts_with('#') && !k.contains(' '))
        .collect();
    assert!(names.len() >= 8, "found settings: {names:?}");
    for name in names {
        let name = name
            .replace("heading2_", "heading1_")
            .replace("heading3_", "heading1_");
        assert!(help.contains(&name), "setting {name} missing from --help");
    }
    for option in [
        "-t",
        "--source",
        "--big-headings",
        "--heading-size",
        "--help",
        "--version",
        "-V",
    ] {
        assert!(help.contains(option), "option {option} missing from --help");
    }
    for place in ["config.toml", "XDG_CONFIG_HOME", "recent_emoji"] {
        assert!(help.contains(place), "{place} missing from --help");
    }
}

#[test]
fn help_fits_in_80_columns() {
    for line in help().lines() {
        assert!(line.chars().count() <= 80, "too wide: {line:?}");
    }
}
