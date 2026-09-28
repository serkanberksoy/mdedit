//! What the terminal can do, detected from its environment variables so
//! mdedit adapts to any terminal. Settings (`config.toml`) can override each
//! detection.

use std::collections::HashMap;

use ratatui::style::Color;
use unicode_width::UnicodeWidthStr;

/// How many colors the terminal can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// 24-bit RGB (`COLORTERM=truecolor`).
    TrueColor,
    /// The xterm 256-color palette (`TERM=*-256color`).
    Indexed256,
    /// The 16 basic ANSI colors (e.g. the Linux console).
    Basic16,
}

/// Terminal features mdedit adapts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub colors: ColorDepth,
    /// DEC double-width / double-height lines (`ESC # 3/4/6`), for larger
    /// headings (R-16). Known to work in Konsole, xterm and WezTerm.
    pub line_sizes: bool,
    /// The terminal and its font can show Unicode symbols (box drawing,
    /// ☐ ☑ …); otherwise mdedit draws ASCII.
    pub unicode: bool,
}

impl Default for Capabilities {
    /// A modern terminal without double-size lines (what tests assume).
    fn default() -> Self {
        Capabilities {
            colors: ColorDepth::TrueColor,
            line_sizes: false,
            unicode: true,
        }
    }
}

impl Capabilities {
    /// Detects capabilities from environment variables (`TERM`,
    /// `COLORTERM`, `KONSOLE_VERSION`, `LANG` …).
    pub fn detect(env: &HashMap<String, String>) -> Self {
        let var = |k: &str| env.get(k).map(String::as_str).filter(|v| !v.is_empty());
        let term = var("TERM").unwrap_or_default();

        let colors = match var("COLORTERM") {
            Some("truecolor" | "24bit") => ColorDepth::TrueColor,
            _ if term.contains("256color") => ColorDepth::Indexed256,
            _ => ColorDepth::Basic16,
        };

        let line_sizes = var("KONSOLE_VERSION").is_some()
            || var("XTERM_VERSION").is_some()
            || var("TERM_PROGRAM") == Some("WezTerm");

        // The locale that applies to characters: LC_ALL, then LC_CTYPE, then LANG.
        let locale = var("LC_ALL").or(var("LC_CTYPE")).or(var("LANG"));
        let utf8_locale = locale.is_none_or(|l| {
            let l = l.to_ascii_lowercase();
            l.contains("utf-8") || l.contains("utf8")
        });
        let unicode = term != "linux" && utf8_locale;

        Capabilities {
            colors,
            line_sizes,
            unicode,
        }
    }

    /// Detects capabilities from this process's environment.
    pub fn from_env() -> Self {
        Self::detect(&std::env::vars().collect())
    }
}

/// `color` as the terminal can show it: RGB colors become the nearest
/// 256-palette or basic ANSI color.
pub fn fit_color(color: Color, depth: ColorDepth) -> Color {
    match (depth, color) {
        (ColorDepth::TrueColor, c) => c,
        (ColorDepth::Indexed256, Color::Rgb(r, g, b)) => Color::Indexed(nearest_256(r, g, b)),
        (ColorDepth::Basic16, Color::Rgb(r, g, b)) => nearest_16(r, g, b),
        (ColorDepth::Basic16, Color::Indexed(i)) => {
            let (r, g, b) = palette_rgb(i);
            nearest_16(r, g, b)
        }
        (_, c) => c,
    }
}

/// The 16 basic colors as RGB (the standard VGA palette).
const BASIC: [(Color, (u8, u8, u8)); 16] = [
    (Color::Black, (0, 0, 0)),
    (Color::Red, (170, 0, 0)),
    (Color::Green, (0, 170, 0)),
    (Color::Yellow, (170, 85, 0)),
    (Color::Blue, (0, 0, 170)),
    (Color::Magenta, (170, 0, 170)),
    (Color::Cyan, (0, 170, 170)),
    (Color::Gray, (170, 170, 170)),
    (Color::DarkGray, (85, 85, 85)),
    (Color::LightRed, (255, 85, 85)),
    (Color::LightGreen, (85, 255, 85)),
    (Color::LightYellow, (255, 255, 85)),
    (Color::LightBlue, (85, 85, 255)),
    (Color::LightMagenta, (255, 85, 255)),
    (Color::LightCyan, (85, 255, 255)),
    (Color::White, (255, 255, 255)),
];

/// Levels of the 6×6×6 color cube in the 256-color palette.
const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn distance((r1, g1, b1): (u8, u8, u8), (r2, g2, b2): (u8, u8, u8)) -> u32 {
    let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2) as u32;
    d(r1, r2) + d(g1, g2) + d(b1, b2)
}

fn nearest_16(r: u8, g: u8, b: u8) -> Color {
    BASIC
        .iter()
        .min_by_key(|(_, rgb)| distance(*rgb, (r, g, b)))
        .map(|(c, _)| *c)
        .expect("the palette isn't empty")
}

/// The nearest 256-palette index: a color-cube entry or a grey-ramp step.
fn nearest_256(r: u8, g: u8, b: u8) -> u8 {
    let level = |v: u8| {
        (0..6)
            .min_by_key(|&k| (i32::from(CUBE[k]) - i32::from(v)).abs())
            .expect("six levels") as u8
    };
    let (cr, cg, cb) = (level(r), level(g), level(b));
    let cube = 16 + 36 * cr + 6 * cg + cb;
    let average = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
    let step = (average.saturating_sub(3) / 10).min(23) as u8;
    let grey = 232 + step;
    if distance(palette_rgb(grey), (r, g, b)) < distance(palette_rgb(cube), (r, g, b)) {
        grey
    } else {
        cube
    }
}

/// The RGB value of a 256-palette index.
fn palette_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0..=15 => BASIC[usize::from(i)].1,
        16..=231 => {
            let n = i - 16;
            (
                CUBE[usize::from(n / 36)],
                CUBE[usize::from(n / 6 % 6)],
                CUBE[usize::from(n % 6)],
            )
        }
        _ => {
            let v = 8 + 10 * (i - 232);
            (v, v, v)
        }
    }
}

/// What to draw for `symbol` on a terminal without Unicode: an ASCII
/// look-alike for mdedit's own symbols, `?` for other characters (`??` if
/// it's two columns wide). `None` if it's already ASCII.
pub fn ascii_fallback(symbol: &str) -> Option<String> {
    if symbol.is_ascii() {
        return None;
    }
    let look_alike = match symbol {
        "│" | "┃" | "▎" => "|",
        "─" | "═" | "┄" | "━" | "—" | "–" => "-",
        "╭" | "╰" | "┌" | "└" | "┐" | "┘" | "┬" | "┴" | "├" | "┼" | "┤" => {
            "+"
        }
        "•" | "◦" | "▪" | "★" | "✦" | "※" => "*",
        "█" | "▌" => "#",
        "☐" => "o",
        "☑" | "☒" | "✘" => "x",
        "⦿" => "@",
        "◐" => "/",
        "➜" | "▸" | "→" | "›" => ">",
        "⏲" | "←" | "‹" => "<",
        "⚑" | "⚠" | "ϟ" => "!",
        "✎" => "~",
        "≡" | "☰" => "=",
        "ℹ" => "i",
        "✔" | "▾" | "↓" => "v",
        "↑" => "^",
        "❝" => "\"",
        "⧉" => "&",
        "⌥" => "A",
        "·" | "…" => ".",
        _ => return Some("?".repeat(symbol.width().max(1))),
    };
    Some(look_alike.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(vars: &[(&str, &str)]) -> Capabilities {
        Capabilities::detect(
            &vars
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn truecolor_keeps_rgb() {
        let c = Color::Rgb(25, 55, 100);
        assert_eq!(fit_color(c, ColorDepth::TrueColor), c);
    }

    #[test]
    fn rgb_to_the_256_palette() {
        assert_eq!(
            fit_color(Color::Rgb(0, 0, 0), ColorDepth::Indexed256),
            Color::Indexed(16)
        );
        assert_eq!(
            fit_color(Color::Rgb(255, 255, 255), ColorDepth::Indexed256),
            Color::Indexed(231)
        );
        assert_eq!(
            fit_color(Color::Rgb(255, 0, 0), ColorDepth::Indexed256),
            Color::Indexed(196)
        );
        assert_eq!(
            fit_color(Color::Rgb(128, 128, 128), ColorDepth::Indexed256),
            Color::Indexed(244),
            "grey ramp"
        );
        assert_eq!(
            fit_color(Color::Cyan, ColorDepth::Indexed256),
            Color::Cyan,
            "named colors stay"
        );
    }

    #[test]
    fn rgb_and_palette_to_basic_colors() {
        // Nearest in the standard VGA palette.
        assert_eq!(
            fit_color(Color::Rgb(250, 10, 10), ColorDepth::Basic16),
            Color::Red
        );
        assert_eq!(
            fit_color(Color::Rgb(25, 55, 100), ColorDepth::Basic16),
            Color::DarkGray
        );
        assert_eq!(
            fit_color(Color::Rgb(192, 197, 206), ColorDepth::Basic16),
            Color::Gray
        );
        assert_eq!(
            fit_color(Color::Indexed(196), ColorDepth::Basic16),
            Color::Red
        );
        assert_eq!(fit_color(Color::Yellow, ColorDepth::Basic16), Color::Yellow);
    }

    #[test]
    fn ascii_look_alikes() {
        let a = |s: &str| ascii_fallback(s).unwrap();
        assert_eq!(a("│"), "|");
        assert_eq!(a("─"), "-");
        assert_eq!(a("╭"), "+");
        assert_eq!(a("•"), "*");
        assert_eq!(a("☐"), "o");
        assert_eq!(a("☑"), "x");
        assert_eq!(a("█"), "#");
        assert_eq!(a("é"), "?");
        assert_eq!(a("😀"), "??", "wide characters keep their width");
        assert_eq!(ascii_fallback("a"), None);
    }

    #[test]
    fn konsole() {
        let c = caps(&[
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("KONSOLE_VERSION", "260800"),
            ("LANG", "en_US.UTF-8"),
        ]);
        assert_eq!(c.colors, ColorDepth::TrueColor);
        assert!(c.line_sizes);
        assert!(c.unicode);
    }

    #[test]
    fn gnome_terminal_and_others_without_line_sizes() {
        let c = caps(&[
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("VTE_VERSION", "7800"),
        ]);
        assert!(!c.line_sizes, "VTE doesn't draw DEC double-size lines");
        assert!(!caps(&[("TERM", "xterm-kitty"), ("COLORTERM", "truecolor")]).line_sizes);
        assert!(!caps(&[("TERM", "alacritty")]).line_sizes);
    }

    #[test]
    fn xterm_and_wezterm_have_line_sizes() {
        assert!(caps(&[("TERM", "xterm"), ("XTERM_VERSION", "XTerm(390)")]).line_sizes);
        assert!(caps(&[("TERM_PROGRAM", "WezTerm")]).line_sizes);
    }

    #[test]
    fn color_depth() {
        assert_eq!(
            caps(&[("COLORTERM", "24bit")]).colors,
            ColorDepth::TrueColor
        );
        assert_eq!(
            caps(&[("TERM", "screen-256color")]).colors,
            ColorDepth::Indexed256
        );
        assert_eq!(caps(&[("TERM", "xterm")]).colors, ColorDepth::Basic16);
        assert_eq!(caps(&[("TERM", "linux")]).colors, ColorDepth::Basic16);
    }

    #[test]
    fn unicode_needs_a_utf8_locale_and_not_the_linux_console() {
        assert!(!caps(&[("TERM", "linux"), ("LANG", "en_US.UTF-8")]).unicode);
        assert!(!caps(&[("TERM", "xterm"), ("LANG", "C")]).unicode);
        assert!(
            !caps(&[
                ("TERM", "xterm"),
                ("LC_ALL", "POSIX"),
                ("LANG", "en_US.UTF-8")
            ])
            .unicode,
            "LC_ALL wins"
        );
        assert!(caps(&[("TERM", "xterm"), ("LC_CTYPE", "de_DE.utf8")]).unicode);
        assert!(
            caps(&[("TERM", "xterm")]).unicode,
            "no locale set: assume UTF-8"
        );
    }
}
