//! User settings from `~/.config/mdedit/config.toml` (or
//! `$XDG_CONFIG_HOME/mdedit/config.toml`). A small subset of TOML: one
//! `key = "value"` per line, `#` comments. Unknown keys or values are
//! reported as warnings and ignored, so a bad line never stops the editor.

use std::path::{Path, PathBuf};

use crate::markdown::{DoneStyle, Options};
use crate::terminal::{Capabilities, ColorDepth};
use ratatui::style::Color;

/// Whether headings use the terminal's double-size lines (R-16).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeadingSize {
    /// If the terminal is known to support them (Konsole, xterm, WezTerm).
    Auto,
    On,
    /// Normal size (the default).
    #[default]
    Off,
}

/// How images are drawn (E-04): `images = "auto" | "kitty" | "sixel" |
/// "iterm2" | "halfblocks" | "off"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Images {
    /// Ask the terminal (half blocks if it has no image protocol).
    #[default]
    Auto,
    Kitty,
    Sixel,
    Iterm2,
    /// Colored `▀` characters: works in any terminal with true color.
    Halfblocks,
    /// Only the image's title row.
    Off,
}

/// All settings, with their defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// `auto_pair = "on" | "off"` (V-04): closing brackets and `**` pairs
    /// are typed for you. On by default.
    pub auto_pair: bool,
    /// `undo_steps = N` (V-18): how many steps Ctrl+Z can undo, 1 to
    /// 10000. 5 by default.
    pub undo_steps: usize,
    /// `images = "auto" | "kitty" | "sixel" | "iterm2" | "halfblocks" |
    /// "off"` (E-04).
    pub images: Images,
    /// How done tasks look (R-07): `"strike"` (default) or `"grey"`.
    pub done_style: DoneStyle,
    /// `heading_size = "auto" | "on" | "off"` (R-16).
    pub heading_size: HeadingSize,
    /// `colors = "auto" | "truecolor" | "256" | "16"`; `None` = detect.
    pub colors: Option<ColorDepth>,
    /// `glyphs = "auto" | "unicode" | "ascii"`: `Some(true)` = Unicode,
    /// `Some(false)` = ASCII, `None` = detect.
    pub glyphs: Option<bool>,
    /// `heading1_color` … `heading6_color`: a color name, `#rrggbb` or a
    /// 256-palette number; `None` keeps the default color for that level.
    pub heading_colors: [Option<Color>; 6],
}

impl Default for Config {
    fn default() -> Self {
        Config {
            auto_pair: true,
            undo_steps: crate::history::History::DEFAULT_LIMIT,
            images: Images::Auto,
            done_style: DoneStyle::default(),
            heading_size: HeadingSize::default(),
            colors: None,
            glyphs: None,
            heading_colors: [None; 6],
        }
    }
}

impl Config {
    /// Parses `text`, returning the settings and a warning per bad line.
    pub fn parse(text: &str) -> (Config, Vec<String>) {
        let mut config = Config::default();
        let mut warnings = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                warnings.push(format!("config line {}: expected key = \"value\"", n + 1));
                continue;
            };
            let value = value.trim().trim_matches('"');
            match (key.trim(), value) {
                ("done_style", "strike") => config.done_style = DoneStyle::Strike,
                ("done_style", "grey" | "gray") => config.done_style = DoneStyle::Grey,
                ("colors", "auto") => config.colors = None,
                ("colors", "truecolor" | "24bit") => config.colors = Some(ColorDepth::TrueColor),
                ("colors", "256") => config.colors = Some(ColorDepth::Indexed256),
                ("colors", "16") => config.colors = Some(ColorDepth::Basic16),
                ("colors", other) => warnings.push(format!(
                    "config line {}: colors must be \"auto\", \"truecolor\", \"256\" or \"16\", not {other:?}",
                    n + 1
                )),
                ("glyphs", "auto") => config.glyphs = None,
                ("glyphs", "unicode") => config.glyphs = Some(true),
                ("glyphs", "ascii") => config.glyphs = Some(false),
                ("glyphs", other) => warnings.push(format!(
                    "config line {}: glyphs must be \"auto\", \"unicode\" or \"ascii\", not {other:?}",
                    n + 1
                )),
                (key, value)
                    if key.starts_with("heading")
                        && key.ends_with("_color")
                        && matches!(key.as_bytes().get(7), Some(b'1'..=b'6'))
                        && key.len() == "heading1_color".len() =>
                {
                    let level = usize::from(key.as_bytes()[7] - b'0');
                    match parse_color(value) {
                        Some(c) => config.heading_colors[level - 1] = Some(c),
                        None => warnings.push(format!(
                            "config line {}: {key} must be a color name, #rrggbb or 0-255, not {value:?}",
                            n + 1
                        )),
                    }
                }
                ("heading_size", "auto") => config.heading_size = HeadingSize::Auto,
                ("heading_size", "on") => config.heading_size = HeadingSize::On,
                ("heading_size", "off") => config.heading_size = HeadingSize::Off,
                ("heading_size", other) => warnings.push(format!(
                    "config line {}: heading_size must be \"auto\", \"on\" or \"off\", not {other:?}",
                    n + 1
                )),
                ("images", "auto") => config.images = Images::Auto,
                ("images", "kitty") => config.images = Images::Kitty,
                ("images", "sixel") => config.images = Images::Sixel,
                ("images", "iterm2") => config.images = Images::Iterm2,
                ("images", "halfblocks") => config.images = Images::Halfblocks,
                ("images", "off") => config.images = Images::Off,
                ("images", other) => warnings.push(format!(
                    "config line {}: images must be \"auto\", \"kitty\", \"sixel\", \"iterm2\", \"halfblocks\" or \"off\", not {other:?}",
                    n + 1
                )),
                ("undo_steps", value) => match value.parse::<usize>() {
                    Ok(n @ 1..=10_000) => config.undo_steps = n,
                    _ => warnings.push(format!(
                        "config line {}: undo_steps must be a number from 1 to 10000, not {value:?}",
                        n + 1
                    )),
                },
                ("auto_pair", "on") => config.auto_pair = true,
                ("auto_pair", "off") => config.auto_pair = false,
                ("auto_pair", other) => warnings.push(format!(
                    "config line {}: auto_pair must be \"on\" or \"off\", not {other:?}",
                    n + 1
                )),
                ("done_style", other) => warnings.push(format!(
                    "config line {}: done_style must be \"strike\" or \"grey\", not {other:?}",
                    n + 1
                )),
                (other, _) => {
                    warnings.push(format!("config line {}: unknown key {other:?}", n + 1))
                }
            }
        }
        (config, warnings)
    }

    /// Loads the file at `path`; a missing file gives the defaults.
    pub fn load(path: &Path) -> (Config, Vec<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Config::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Config::default(), Vec::new()),
            Err(e) => (
                Config::default(),
                vec![format!("cannot read {}: {e}", path.display())],
            ),
        }
    }

    /// The terminal capabilities after applying the `colors` / `glyphs`
    /// overrides to what was detected.
    pub fn apply(&self, detected: Capabilities) -> Capabilities {
        Capabilities {
            colors: self.colors.unwrap_or(detected.colors),
            unicode: self.glyphs.unwrap_or(detected.unicode),
            ..detected
        }
    }

    /// The rendering options these settings imply for a terminal that can
    /// do `caps`.
    pub fn options(&self, caps: &Capabilities) -> Options {
        Options {
            done_style: self.done_style,
            heading_sizes: match self.heading_size {
                HeadingSize::Auto => caps.line_sizes,
                HeadingSize::On => true,
                HeadingSize::Off => false,
            },
            heading_colors: self.heading_colors,
            source_mode: false,
        }
    }
}

/// A color from settings: a name (`magenta`, `light blue`, `dark-gray` …),
/// `#rrggbb`, or a 256-palette number.
pub fn parse_color(value: &str) -> Option<Color> {
    let name: String = value
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect();
    let named = match name.as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ => {
            if let Some(hex) = name.strip_prefix('#')
                && hex.len() == 6
            {
                let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
                return Some(Color::Rgb(channel(0)?, channel(2)?, channel(4)?));
            }
            return name.parse::<u8>().ok().map(Color::Indexed);
        }
    };
    Some(named)
}

/// `$XDG_CONFIG_HOME/mdedit/config.toml`, or `~/.config/mdedit/config.toml`.
pub fn default_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config.join("mdedit").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_strikethrough() {
        assert_eq!(Config::default().done_style, DoneStyle::Strike);
        assert_eq!(Config::parse("").0, Config::default());
    }

    #[test]
    fn reads_done_style() {
        let (c, warnings) = Config::parse("# my settings\n\ndone_style = \"grey\"\n");
        assert_eq!(c.done_style, DoneStyle::Grey);
        assert!(warnings.is_empty());
        assert_eq!(
            Config::parse("done_style=strike").0.done_style,
            DoneStyle::Strike
        );
    }

    #[test]
    fn bad_lines_warn_and_are_ignored() {
        let (c, warnings) = Config::parse("done_style = \"purple\"\ncolour = \"x\"\nnonsense");
        assert_eq!(c, Config::default());
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings[0].contains("line 1"), "{warnings:?}");
    }

    #[test]
    fn reads_images() {
        assert_eq!(Config::default().images, Images::Auto);
        assert_eq!(Config::parse("images = \"sixel\"").0.images, Images::Sixel);
        assert_eq!(Config::parse("images = off").0.images, Images::Off);
        assert_eq!(
            Config::parse("images = halfblocks").0.images,
            Images::Halfblocks
        );
        let (c, warnings) = Config::parse("images = png");
        assert_eq!(c.images, Images::Auto);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn reads_undo_steps() {
        assert_eq!(Config::default().undo_steps, 5);
        assert_eq!(Config::parse("undo_steps = 50").0.undo_steps, 50);
        for bad in ["undo_steps = 0", "undo_steps = lots", "undo_steps = 100001"] {
            let (c, warnings) = Config::parse(bad);
            assert_eq!(c.undo_steps, 5, "{bad}");
            assert_eq!(warnings.len(), 1, "{bad}");
        }
    }

    #[test]
    fn reads_auto_pair() {
        assert!(Config::default().auto_pair, "on by default");
        assert!(!Config::parse("auto_pair = \"off\"").0.auto_pair);
        assert!(Config::parse("auto_pair = on").0.auto_pair);
        let (c, warnings) = Config::parse("auto_pair = maybe");
        assert!(c.auto_pair);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn reads_heading_size() {
        assert_eq!(
            Config::default().heading_size,
            HeadingSize::Off,
            "normal size by default"
        );
        let (c, w) = Config::parse("heading_size = \"off\"");
        assert_eq!((c.heading_size, w.len()), (HeadingSize::Off, 0));
        assert_eq!(
            Config::parse("heading_size = on").0.heading_size,
            HeadingSize::On
        );
        assert_eq!(Config::parse("heading_size = big").1.len(), 1);
    }

    #[test]
    fn reads_color_and_glyph_overrides() {
        use crate::terminal::ColorDepth;
        let (c, w) = Config::parse("colors = \"256\"\nglyphs = \"ascii\"");
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.colors, Some(ColorDepth::Indexed256));
        assert_eq!(c.glyphs, Some(false));
        let (c, _) = Config::parse("colors = auto\nglyphs = auto");
        assert_eq!((c.colors, c.glyphs), (None, None));
        assert_eq!(Config::parse("colors = 12").1.len(), 1);
    }

    #[test]
    fn overrides_apply_on_top_of_detection() {
        use crate::terminal::{Capabilities, ColorDepth};
        let (c, _) = Config::parse("colors = 16\nglyphs = ascii");
        let caps = c.apply(Capabilities::default());
        assert_eq!(caps.colors, ColorDepth::Basic16);
        assert!(!caps.unicode);
        assert_eq!(
            Config::default().apply(Capabilities::default()),
            Capabilities::default()
        );
    }

    #[test]
    fn reads_heading_colors() {
        let (c, w) = Config::parse(
            "heading1_color = \"red\"\nheading2_color = \"#ff8800\"\nheading3_color = 208\nheading4_color = \"light blue\"",
        );
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.heading_colors[0], Some(Color::Red));
        assert_eq!(c.heading_colors[1], Some(Color::Rgb(255, 136, 0)));
        assert_eq!(c.heading_colors[2], Some(Color::Indexed(208)));
        assert_eq!(c.heading_colors[3], Some(Color::LightBlue));
        assert_eq!(c.heading_colors[4], None, "unset: the default color");
        assert_eq!(Config::parse("heading7_color = red").1.len(), 1);
        assert_eq!(Config::parse("heading1_color = \"#12\"").1.len(), 1);
        assert_eq!(Config::parse("heading1_color = mauve").1.len(), 1);
    }

    #[test]
    fn heading_colors_reach_the_render_options() {
        let (c, _) = Config::parse("heading2_color = green");
        let o = c.options(&Capabilities::default());
        assert_eq!(o.heading_colors[1], Some(Color::Green));
    }

    #[test]
    fn load_missing_file_gives_defaults() {
        let (c, warnings) = Config::load(Path::new("/nonexistent/mdedit.toml"));
        assert_eq!(c, Config::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn default_path_is_in_the_config_folder() {
        let p = default_path().unwrap();
        assert!(p.ends_with("mdedit/config.toml"), "{}", p.display());
    }
}
