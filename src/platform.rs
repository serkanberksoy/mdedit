//! Where things are and how things are run, on Linux, macOS and Windows:
//! the home and config folders, the program that opens a file or a link,
//! the shell that runs a command line, and the clipboard's tools. Each is
//! worked out from the system ([`Os`]) and the environment given, so every
//! system's answer can be tested anywhere; the plain functions use this
//! system's.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

/// A kind of system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
}

impl Os {
    /// The system this runs on (other Unix systems as Linux).
    pub fn this() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// The home folder: `HOME`, else (Windows) `USERPROFILE`.
pub fn home_in(os: Os, env: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    env("HOME")
        .filter(|h| !h.is_empty())
        .or_else(|| (os == Os::Windows).then(|| env("USERPROFILE")).flatten())
        .map(PathBuf::from)
}

/// The folder settings go in: `XDG_CONFIG_HOME`, else `%APPDATA%`
/// (Windows), else `~/.config`.
pub fn config_home_in(os: Os, env: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    env("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            (os == Os::Windows)
                .then(|| env("APPDATA").map(PathBuf::from))
                .flatten()
        })
        .or_else(|| home_in(os, env).map(|h| h.join(".config")))
}

fn env(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// This system's home folder.
pub fn home() -> Option<PathBuf> {
    home_in(Os::this(), &env)
}

/// This system's settings folder (blackglass's and mdedit's are in it).
pub fn config_home() -> Option<PathBuf> {
    config_home_in(Os::this(), &env)
}

/// A Windows path without its verbatim prefix, where the plain form says
/// the same: `\\?\C:\x` is `C:\x`, `\\?\UNC\host\share` is
/// `\\host\share` (Windows' canonical paths have the prefix; people and
/// other programs read the plain form).
pub fn plain_windows_path(text: &str) -> &str {
    let Some(rest) = text.strip_prefix(r"\\?\") else {
        return text;
    };
    let bytes = rest.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        rest
    } else {
        text
    }
}

/// `path` made absolute with its links followed, as
/// [`std::fs::canonicalize`], but on Windows in the plain form
/// ([`plain_windows_path`]; UNC shares keep the prefix).
pub fn canonical(path: &std::path::Path) -> std::io::Result<PathBuf> {
    let full = std::fs::canonicalize(path)?;
    if cfg!(windows) {
        let text = full.to_string_lossy();
        let plain = plain_windows_path(&text);
        if plain.len() != text.len() {
            return Ok(PathBuf::from(plain));
        }
    }
    Ok(full)
}

/// The program and arguments that open `target` (a file or a web
/// address) in its own program.
pub fn open_args(os: Os, target: &str) -> (String, Vec<String>) {
    match os {
        Os::Linux => ("xdg-open".into(), vec![target.into()]),
        Os::MacOs => ("open".into(), vec![target.into()]),
        // `start`'s first quoted argument is a title: an empty one.
        Os::Windows => (
            "cmd".into(),
            vec!["/C".into(), "start".into(), String::new(), target.into()],
        ),
    }
}

/// The program and arguments that run the command line `line`.
pub fn shell_args(os: Os, line: &str) -> (String, Vec<String>) {
    match os {
        Os::Windows => ("cmd".into(), vec!["/C".into(), line.into()]),
        _ => ("sh".into(), vec!["-c".into(), line.into()]),
    }
}

fn command((program, args): (String, Vec<String>)) -> Command {
    let mut c = Command::new(program);
    c.args(args);
    c
}

/// A command that opens `target` in its own program, on this system.
pub fn open(target: &str) -> Command {
    command(open_args(Os::this(), target))
}

/// A command that runs the command line `line`, on this system.
pub fn shell(line: &str) -> Command {
    command(shell_args(Os::this(), line))
}

/// PowerShell, reading and writing UTF-8 (the console's code page may
/// not be).
const POWERSHELL_COPY: &str = "[Console]::InputEncoding=[Text.Encoding]::UTF8; Set-Clipboard -Value ([Console]::In.ReadToEnd())";
const POWERSHELL_PASTE: &str =
    "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-Clipboard -Raw";

/// The tools that put text (on their input) on the clipboard, in the
/// order they're tried.
pub fn copy_tools(os: Os) -> Vec<Vec<String>> {
    let list: &[&[&str]] = match os {
        Os::Linux => &[
            &["wl-copy"],
            &["xclip", "-selection", "clipboard"],
            &["xsel", "--clipboard", "--input"],
        ],
        Os::MacOs => &[&["pbcopy"]],
        Os::Windows => &[&["powershell", "-NoProfile", "-Command", POWERSHELL_COPY]],
    };
    list.iter()
        .map(|t| t.iter().map(|s| s.to_string()).collect())
        .collect()
}

/// The tools that write the clipboard's text, in the order they're tried.
pub fn paste_tools(os: Os) -> Vec<Vec<String>> {
    let list: &[&[&str]] = match os {
        Os::Linux => &[
            &["wl-paste", "--no-newline"],
            &["xclip", "-selection", "clipboard", "-o"],
            &["xsel", "--clipboard", "--output"],
        ],
        Os::MacOs => &[&["pbpaste"]],
        Os::Windows => &[&["powershell", "-NoProfile", "-Command", POWERSHELL_PASTE]],
    };
    list.iter()
        .map(|t| t.iter().map(|s| s.to_string()).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn the_config_folder_on_each_system() {
        let unix = env_of(&[("HOME", "/home/ada")]);
        assert_eq!(
            config_home_in(Os::Linux, &unix),
            Some(PathBuf::from("/home/ada/.config"))
        );
        let xdg = env_of(&[("HOME", "/home/ada"), ("XDG_CONFIG_HOME", "/cfg")]);
        assert_eq!(config_home_in(Os::MacOs, &xdg), Some(PathBuf::from("/cfg")));
        let windows = env_of(&[
            ("USERPROFILE", "C:\\Users\\Ada"),
            ("APPDATA", "C:\\Users\\Ada\\AppData\\Roaming"),
        ]);
        assert_eq!(
            config_home_in(Os::Windows, &windows),
            Some(PathBuf::from("C:\\Users\\Ada\\AppData\\Roaming"))
        );
        assert_eq!(
            home_in(Os::Windows, &windows),
            Some(PathBuf::from("C:\\Users\\Ada"))
        );
        assert_eq!(
            home_in(Os::Linux, &windows),
            None,
            "USERPROFILE is Windows'"
        );
        assert_eq!(config_home_in(Os::Linux, &env_of(&[])), None);
    }

    #[test]
    fn opening_running_and_the_clipboard_on_each_system() {
        assert_eq!(
            open_args(Os::Windows, "https://x.io"),
            (
                "cmd".to_string(),
                vec![
                    "/C".into(),
                    "start".into(),
                    String::new(),
                    "https://x.io".into()
                ]
            )
        );
        assert_eq!(open_args(Os::Linux, "a.pdf").0, "xdg-open");
        assert_eq!(open_args(Os::MacOs, "a.pdf").0, "open");
        assert_eq!(
            shell_args(Os::Windows, "git status"),
            (
                "cmd".to_string(),
                vec!["/C".to_string(), "git status".to_string()]
            )
        );
        assert_eq!(shell_args(Os::Linux, "ls").0, "sh");
        assert_eq!(copy_tools(Os::Windows)[0][0], "powershell");
        assert_eq!(paste_tools(Os::MacOs), [vec!["pbpaste".to_string()]]);
        assert_eq!(copy_tools(Os::Linux)[0], ["wl-copy"]);
    }

    #[test]
    fn windows_paths_without_their_verbatim_prefix() {
        assert_eq!(plain_windows_path(r"\\?\C:\Users\Ada"), r"C:\Users\Ada");
        assert_eq!(plain_windows_path(r"C:\x"), r"C:\x");
        assert_eq!(
            plain_windows_path(r"\\?\UNC\host\share"),
            r"\\?\UNC\host\share",
            "a share keeps it"
        );
        assert_eq!(plain_windows_path("/home/ada"), "/home/ada");
    }

    #[test]
    fn canonical_paths_are_absolute() {
        let here = canonical(std::path::Path::new(".")).unwrap();
        assert!(here.is_absolute());
        assert!(!here.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn this_systems_answers_run() {
        // A command line through this system's shell.
        let out = shell("echo hi").output().expect("the shell runs");
        assert!(String::from_utf8_lossy(&out.stdout).contains("hi"));
    }
}
