//! File dialogs of the mdedit program: the folder browser behind Save As
//! (F-02) and Open (F-04), the dialogs that can be on top of the editor,
//! and saving files safely (F-07).

use std::fs;
use std::path::{Path, PathBuf};

/// The dialog on top of the editor, if any.
#[derive(Debug, Default)]
pub enum Mode {
    #[default]
    Edit,
    /// `Save changes to <name>? [Y]es [N]o [C]ancel`, then do `then`.
    SavePrompt { then: After },
    /// The folder browser (Save As or Open).
    Browser(Browser),
    /// `Overwrite <name>? (y/n)` for a Save As target that exists.
    ConfirmOverwrite { path: PathBuf, browser: Browser },
}

/// What happens after a successful save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// Back to editing.
    Edit,
    /// Exit mdedit (F-03).
    Quit,
    /// Show the Open file picker (F-04).
    Open,
    /// Follow the link that was under the cursor (F-05).
    FollowLink,
}

/// Why the browser is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    SaveAs { then: After },
    Open,
}

/// One row of the browser's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
}

/// The folder browser shared by Save As and Open.
#[derive(Debug)]
pub struct Browser {
    pub purpose: Purpose,
    pub dir: PathBuf,
    /// Save As: the file name (or a path). Open: a name filter (or a path).
    pub input: String,
    /// `..`, then folders, then files (Markdown only unless `show_all`).
    pub entries: Vec<Entry>,
    /// Highlighted list row; `None` means the input field has focus.
    pub selected: Option<usize>,
    pub show_all: bool,
    pub error: Option<String>,
}

impl Browser {
    pub fn new(purpose: Purpose, dir: PathBuf, input: String) -> Self {
        let mut b = Browser {
            purpose,
            dir,
            input,
            entries: Vec::new(),
            selected: None,
            show_all: false,
            error: None,
        };
        b.refresh();
        b
    }

    /// Re-reads the folder, applying the Open filter and the `show_all` toggle.
    pub fn refresh(&mut self) {
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        match fs::read_dir(&self.dir) {
            Ok(read) => {
                for entry in read.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let is_dir = entry.path().is_dir();
                    if !self.show_all && name.starts_with('.') {
                        continue;
                    }
                    if is_dir {
                        dirs.push(name);
                    } else if self.show_all || is_markdown(&name) {
                        files.push(name);
                    }
                }
            }
            Err(e) => self.error = Some(format!("Cannot read {}: {e}", self.dir.display())),
        }
        let filter = match self.purpose {
            Purpose::Open if !is_path_like(&self.input) => self.input.to_lowercase(),
            _ => String::new(),
        };
        let keep = |n: &String| n.to_lowercase().contains(&filter);
        dirs.retain(keep);
        files.retain(keep);
        dirs.sort_by_key(|n| n.to_lowercase());
        files.sort_by_key(|n| n.to_lowercase());

        self.entries.clear();
        if self.dir.parent().is_some() {
            self.entries.push(Entry {
                name: "..".into(),
                is_dir: true,
            });
        }
        self.entries
            .extend(dirs.into_iter().map(|name| Entry { name, is_dir: true }));
        self.entries.extend(files.into_iter().map(|name| Entry {
            name,
            is_dir: false,
        }));

        self.selected = match self.purpose {
            // Open: highlight the first match, skipping `..`.
            Purpose::Open => {
                let first = self.entries.iter().position(|e| e.name != "..");
                first.or((!self.entries.is_empty()).then_some(0))
            }
            Purpose::SaveAs { .. } => None,
        };
    }

    pub fn move_down(&mut self) {
        let last = self.entries.len().checked_sub(1);
        self.selected = match (self.selected, last) {
            (_, None) => None,
            (None, Some(_)) => Some(0),
            (Some(i), Some(last)) => Some((i + 1).min(last)),
        };
    }

    /// Up from the first row returns focus to the input field (Save As).
    pub fn move_up(&mut self) {
        self.selected = match (self.selected, self.purpose) {
            (Some(0), Purpose::SaveAs { .. }) => None,
            (Some(i), _) => Some(i.saturating_sub(1)),
            (None, _) => None,
        };
    }

    /// Shows `dir`. The Open filter is cleared.
    pub fn enter(&mut self, dir: PathBuf) {
        self.dir = dir;
        self.error = None;
        if self.purpose == Purpose::Open {
            self.input.clear();
        }
        self.refresh();
    }

    /// The highlighted entry's full path (`..` resolves to the parent).
    pub fn selected_path(&self) -> Option<(PathBuf, bool)> {
        let e = self.entries.get(self.selected?)?;
        let path = if e.name == ".." {
            self.dir.parent()?.to_path_buf()
        } else {
            self.dir.join(&e.name)
        };
        Some((path, e.is_dir))
    }

    /// The typed input as a path: `~/…` is under the home folder, absolute
    /// paths stay, anything else is relative to the current folder.
    pub fn typed_path(&self) -> PathBuf {
        let input = self.input.trim();
        if let Some(rest) = input.strip_prefix("~/")
            && let Some(home) = std::env::var_os("HOME")
        {
            return Path::new(&home).join(rest);
        }
        self.dir.join(input)
    }
}

/// Writes `contents` to `path` safely (F-07): into a temporary file in the
/// same folder, which then replaces `path` in one step, so a failure never
/// leaves a half-written note. A symbolic link is followed (the link stays)
/// and an existing file keeps its permissions.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = match target.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let tmp = dir.join(format!(".{name}.mdedit-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
        if let Ok(meta) = fs::metadata(&target) {
            file.set_permissions(meta.permissions())?;
        }
        file.sync_all()?;
        fs::rename(&tmp, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn is_markdown(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// Whether Open input should be treated as a path rather than a filter.
pub fn is_path_like(input: &str) -> bool {
    input.contains('/') || input.starts_with('~')
}
