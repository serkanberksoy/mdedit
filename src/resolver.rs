//! How links, embeds and images find their files. mdedit resolves them
//! relative to the note ([`RelativeResolver`]); a host that embeds mdedit
//! (e.g. an app with a vault) can plug in its own [`Resolver`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Finds the files that links (`[[Note]]`, `[text](Note.md)`), note
/// embeds (`![[Note]]`) and image embeds (`![[photo.png]]`) point to.
pub trait Resolver {
    /// The file that link `target` (as written, `%20` decoded: `Note`,
    /// `sub/Note.md`, `photo.png`) in the note at `from` points to. `from`
    /// is `None` for an untitled note. `Err` says what was looked for, for
    /// the status bar.
    fn resolve(&self, from: Option<&Path>, target: &str) -> Result<PathBuf, String>;

    /// Whether `target` points to an existing file (for styling unresolved
    /// links).
    fn exists(&self, from: Option<&Path>, target: &str) -> bool {
        self.resolve(from, target).is_ok()
    }

    /// The lines of the note at `path`, for embeds. The default reads the
    /// file (cached until it changes).
    fn load(&self, path: &Path) -> Option<Arc<Vec<String>>> {
        crate::embed::load(path)
    }
}

/// mdedit's own resolver: a target is a path relative to the note's folder
/// (or `cwd` for an untitled note), with `.md` added if needed. No vault.
#[derive(Debug, Clone)]
pub struct RelativeResolver {
    /// The folder for untitled notes.
    pub cwd: PathBuf,
}

impl RelativeResolver {
    /// Resolves relative to the current working folder for untitled notes.
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        RelativeResolver { cwd }
    }
}

impl Default for RelativeResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver for RelativeResolver {
    fn resolve(&self, from: Option<&Path>, target: &str) -> Result<PathBuf, String> {
        let base = from
            .and_then(Path::parent)
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(&self.cwd);
        crate::links::resolve(base, target).map_err(|missing| missing.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join("mdedit-resolver-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        d
    }

    #[test]
    fn targets_are_relative_to_the_note() {
        let d = scratch("relative");
        std::fs::write(d.join("sub").join("b.md"), "").unwrap();
        std::fs::write(d.join("sub").join("pic.png"), "").unwrap();
        let r = RelativeResolver { cwd: d.clone() };
        let note = d.join("sub").join("a.md");
        assert_eq!(r.resolve(Some(&note), "b"), Ok(d.join("sub").join("b.md")));
        assert_eq!(
            r.resolve(Some(&note), "pic.png"),
            Ok(d.join("sub").join("pic.png"))
        );
        assert_eq!(
            r.resolve(None, "sub/b"),
            Ok(d.join("sub").join("b.md")),
            "untitled: cwd"
        );
        let missing = r.resolve(Some(&note), "nope").unwrap_err();
        assert!(missing.ends_with("nope.md"), "{missing}");
        assert!(r.exists(Some(&note), "b") && !r.exists(Some(&note), "nope"));
    }

    #[test]
    fn a_note_without_a_folder_uses_cwd() {
        let d = scratch("bare");
        std::fs::write(d.join("b.md"), "").unwrap();
        let r = RelativeResolver { cwd: d.clone() };
        assert_eq!(r.resolve(Some(Path::new("a.md")), "b"), Ok(d.join("b.md")));
    }

    #[test]
    fn new_uses_the_working_folder() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(RelativeResolver::new().cwd, cwd);
    }
}
