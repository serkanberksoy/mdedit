//! A host's colors for the editor (a theme). mdedit draws with the 16
//! named terminal colors and the terminal's own text and background; a
//! [`Palette`] swaps them for others, like a terminal color scheme, when
//! [`crate::ui::EditorWidget`] draws. The default changes nothing.

use ratatui::style::Color;

/// The named colors, in [`Palette::named`] order.
pub const NAMED: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

/// Colors to draw instead of the editor's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Palette {
    /// For each named color (in [`NAMED`] order), what to draw instead.
    pub named: [Option<Color>; 16],
    /// Text with no color of its own (the terminal's default).
    pub text: Option<Color>,
    /// The background where nothing else is (the terminal's default).
    pub background: Option<Color>,
}

impl Palette {
    /// Draws `to` wherever the editor uses named color `from`.
    pub fn set(&mut self, from: Color, to: Color) {
        if let Some(i) = NAMED.iter().position(|c| *c == from) {
            self.named[i] = Some(to);
        }
    }

    /// Whether it changes anything.
    pub fn is_empty(&self) -> bool {
        *self == Palette::default()
    }

    /// A foreground color, swapped.
    pub fn fg(&self, color: Color) -> Color {
        match color {
            Color::Reset => self.text.unwrap_or(Color::Reset),
            c => self.named_or(c),
        }
    }

    /// A background color, swapped.
    pub fn bg(&self, color: Color) -> Color {
        match color {
            Color::Reset => self.background.unwrap_or(Color::Reset),
            c => self.named_or(c),
        }
    }

    fn named_or(&self, color: Color) -> Color {
        NAMED
            .iter()
            .position(|c| *c == color)
            .and_then(|i| self.named[i])
            .unwrap_or(color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_colors_and_defaults_are_swapped() {
        let mut p = Palette::default();
        assert!(p.is_empty());
        assert_eq!(p.fg(Color::Cyan), Color::Cyan);
        p.set(Color::Cyan, Color::Rgb(0, 1, 2));
        p.text = Some(Color::Black);
        assert_eq!(p.fg(Color::Cyan), Color::Rgb(0, 1, 2));
        assert_eq!(p.bg(Color::Cyan), Color::Rgb(0, 1, 2));
        assert_eq!(p.fg(Color::Reset), Color::Black);
        assert_eq!(p.bg(Color::Reset), Color::Reset, "no background set");
        assert_eq!(p.fg(Color::Rgb(9, 9, 9)), Color::Rgb(9, 9, 9), "RGB stays");
    }
}
