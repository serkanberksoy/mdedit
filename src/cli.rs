//! Command-line arguments: `mdedit [OPTIONS] [FILE[#Heading]]`.

use crate::config::HeadingSize;

/// Usage text for `--help`: options, keys and settings (kept in step with
/// the README by `tests/project.rs`).
pub const USAGE: &str = "\
Usage: mdedit [OPTIONS] [FILE[#Heading]]

Edit a Markdown file in the terminal with a live preview: every line is
shown formatted, except the line with the cursor, which is raw Markdown.
FILE#Heading opens at that heading. A FILE that doesn't exist yet is created
when you save.

Options:
  -t, --source                Start in source mode (every line raw);
                              Alt+V switches to view mode
  --big-headings              Same as --heading-size=on
  --heading-size=auto|on|off  Double-size level-1/2 headings (Konsole, xterm,
                              WezTerm); default: off, or heading_size
  -h, --help                  Show this help
  -V, --version               Show the version
  --                          What follows is the file name, even if it
                              starts with -

Keys: moving and selecting
  Arrows, Home, End           Move (Up/Down move by screen row)
  PgUp / PgDn                 Move a screen up / down
  Shift + a move key          Select text (selected lines are shown raw)
  Ctrl+A                      Select everything; Esc ends a selection

Keys: editing
  Enter                       New line; continues lists and tasks (and the
                              task type), ends the list on an empty item
  Tab / Shift+Tab             Nest / un-nest list items (all selected lines)
  Backspace / Delete          Delete (the selection, or an empty pair)
  Ctrl+V                      Paste from the clipboard (wl-paste, xclip,
                              xsel, pbpaste or PowerShell); the terminal's
                              own paste works too
  Ctrl+T                      Turn the line into a task
  Ctrl+L                      Close the task, or reopen a closed one
  Ctrl+Z / Ctrl+Y             Undo / redo (Ctrl+Shift+Z also redoes); typing
                              undoes a word at a time; see undo_steps
  Ctrl+E                      Emoji picker: type to search, arrows, Enter;
                              Esc or Ctrl+E closes it
  With a selection: * _ ~ = ` \" ( [ {
                              Wrap it (* twice: bold); it stays selected
  Paste a URL over a selection
                              Make it a link: [selection](url)
  ( [ { ` and ** == ~~        Auto-pair: the closer is added, typing it
                              steps over it; see auto_pair

Keys: search
  Ctrl+F                      Search as you type (all matches highlighted);
                              Up/Down previous / next match, Enter stays
                              there, Esc goes back
  F3 / Shift+F3               Next / previous match of the last search
  Ctrl+H (or Ctrl+R)          Find and replace: Tab switches fields, Enter
                              replaces and goes to the next, Up/Down skip,
                              Ctrl+A replaces all, Esc closes

Keys: view, links and files
  Alt+V                       Cycle the modes: live preview, source mode
                              (every line raw), view mode (read-only,
                              every line rendered)
  View mode keys              Up/Down/PgUp/PgDn move over rendered rows,
                              Tab / Shift+Tab go to the next / previous
                              link, Enter (or a click) follows it, Esc
                              edits again; typing changes nothing
  Ctrl+K                      Fold / unfold a section, list item or callout
  Ctrl+Enter or Alt+Enter     Follow the link under the cursor ([[Note]],
                              [[#Heading]], [text](file.md))
  Ctrl+O                      Open a file (asks to save changes first)
  Ctrl+S                      Save (an untitled document asks for a name)
  Ctrl+Alt+S or Ctrl+Shift+S  Save as (in Konsole use Ctrl+Alt+S)
  Ctrl+X                      Exit (asks to save unsaved changes)

Settings, one key = \"value\" per line in ~/.config/mdedit/config.toml
(or $XDG_CONFIG_HOME/mdedit/config.toml; %APPDATA%\\mdedit\\config.toml
on Windows):
  done_style = strike | grey          How done tasks look (default: strike)
  auto_pair = on | off                Auto-pair brackets and ** == ~~
                                      (default: on)
  undo_steps = 1 to 10000             Steps Ctrl+Z can undo (default: 1000)
  indent_width = 1 to 8               Spaces Tab indents a list item, and
                                      per list level (default: 2)
  images = auto | kitty | sixel | iterm2 | halfblocks | off
                                      How images are drawn (default: auto,
                                      asks the terminal)
  colors = auto | truecolor | 256 | 16
                                      Colors (default: auto, detected)
  glyphs = auto | unicode | ascii     Symbols (default: auto, detected)
  heading_size = off | on | auto      Double-size headings (default: off)
  heading1_color ... heading6_color = a name, #rrggbb or 0-255
                                      Heading colors (default: built in)

Recently used emoji are kept in ~/.config/mdedit/recent_emoji.
";

/// What the command line asks for.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cli {
    /// The file argument, as typed (may end in `#Heading`).
    pub file: Option<String>,
    /// `--heading-size` / `--big-headings`, overriding `config.toml`.
    pub heading_size: Option<HeadingSize>,
    /// `-t` / `--source`: start in source (text) mode (V-13).
    pub source: bool,
    pub help: bool,
    pub version: bool,
}

/// Parses the arguments after the program name. `Err` explains a bad one.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Cli, String> {
    let mut cli = Cli::default();
    let mut args = args.into_iter();
    let mut options_done = false;
    let size = |v: &str| match v {
        "auto" => Ok(HeadingSize::Auto),
        "on" => Ok(HeadingSize::On),
        "off" => Ok(HeadingSize::Off),
        other => Err(format!(
            "--heading-size must be auto, on or off, not {other:?}"
        )),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            _ if options_done || !arg.starts_with('-') || arg == "-" => {
                if cli.file.replace(arg).is_some() {
                    return Err("only one file can be opened".into());
                }
            }
            "--" => options_done = true,
            "-h" | "--help" => cli.help = true,
            "-t" | "--source" => cli.source = true,
            "-V" | "--version" => cli.version = true,
            "--big-headings" => cli.heading_size = Some(HeadingSize::On),
            "--heading-size" => {
                let value = args.next().ok_or("--heading-size needs auto, on or off")?;
                cli.heading_size = Some(size(&value)?);
            }
            _ => match arg.strip_prefix("--heading-size=") {
                Some(value) => cli.heading_size = Some(size(value)?),
                None => return Err(format!("unknown option {arg:?} (see --help)")),
            },
        }
    }
    Ok(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(args: &[&str]) -> Result<Cli, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn just_a_file() {
        let c = cli(&["note.md#Top"]).unwrap();
        assert_eq!(c.file.as_deref(), Some("note.md#Top"));
        assert_eq!(c.heading_size, None);
        assert_eq!(cli(&[]).unwrap(), Cli::default());
    }

    #[test]
    fn heading_size_options() {
        assert_eq!(
            cli(&["--big-headings", "a.md"]).unwrap().heading_size,
            Some(HeadingSize::On)
        );
        assert_eq!(
            cli(&["--heading-size=auto"]).unwrap().heading_size,
            Some(HeadingSize::Auto)
        );
        assert_eq!(
            cli(&["--heading-size", "off", "a.md"])
                .unwrap()
                .heading_size,
            Some(HeadingSize::Off)
        );
        assert!(cli(&["--heading-size=huge"]).is_err());
        assert!(cli(&["--heading-size"]).is_err(), "missing value");
    }

    #[test]
    fn t_starts_in_source_mode() {
        assert!(cli(&["-t", "a.md"]).unwrap().source);
        assert!(cli(&["--source"]).unwrap().source);
        assert!(!cli(&["a.md"]).unwrap().source);
        assert!(USAGE.contains("-t, --source"));
        assert!(USAGE.contains("-h, --help"));
    }

    #[test]
    fn help_and_version() {
        assert!(cli(&["--help"]).unwrap().help);
        assert!(cli(&["-h"]).unwrap().help);
        assert!(cli(&["--version"]).unwrap().version);
        assert!(cli(&["-V"]).unwrap().version);
    }

    #[test]
    fn unknown_options_and_extra_files_are_errors() {
        assert!(cli(&["--nope"]).is_err());
        assert!(cli(&["a.md", "b.md"]).is_err());
    }

    #[test]
    fn double_dash_ends_options() {
        assert_eq!(
            cli(&["--", "--weird-name.md"]).unwrap().file.as_deref(),
            Some("--weird-name.md")
        );
    }
}
