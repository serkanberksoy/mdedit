//! Syntax highlighting for fenced code blocks (B-09), using syntect's
//! built-in syntaxes and themes (pure-Rust regex engine). Only foreground
//! colors are used, so the terminal's background shows through.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use ratatui::style::{Color, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Theme used for code colors (readable on dark terminal backgrounds).
const THEME: &str = "base16-ocean.dark";

/// syntect's syntaxes and theme, loaded once on first use (~tens of ms).
fn assets() -> &'static (SyntaxSet, Theme) {
    static ASSETS: OnceLock<(SyntaxSet, Theme)> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let syntaxes = SyntaxSet::load_defaults_newlines();
        let mut themes = ThemeSet::load_defaults();
        let theme = themes
            .themes
            .remove(THEME)
            .expect("syntect ships the base16-ocean.dark theme");
        (syntaxes, theme)
    })
}

/// The syntax for a fence info word: a name (`Rust`), token (`rust`) or
/// file extension (`py`), case-insensitive.
fn syntax_for<'a>(syntaxes: &'a SyntaxSet, lang: &str) -> Option<&'a SyntaxReference> {
    if lang.is_empty() {
        return None;
    }
    syntaxes
        .find_syntax_by_token(lang)
        .or_else(|| syntaxes.find_syntax_by_token(&lang.to_lowercase()))
        .or_else(|| syntaxes.find_syntax_by_extension(&lang.to_lowercase()))
        .filter(|s| s.name != "Plain Text")
}

/// Highlighted blocks by (language, text) hash. Highlighting is slow
/// (~0.1 ms per line in release builds) and runs every frame, so blocks are
/// only re-highlighted when their text changes. Cleared when it grows big.
type Cache = HashMap<u64, Arc<Highlighted>>;

/// A highlighted block: styled pieces for each line.
pub type Highlighted = Vec<Vec<(Style, String)>>;

#[cfg(test)]
thread_local! {
    /// How many blocks this thread highlighted (for tests of laziness).
    pub static CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
const CACHE_LIMIT: usize = 256;

/// [`highlight`], memoized by the language and the block's text.
pub fn highlight_cached(lang: &str, lines: &[&str]) -> Option<Arc<Highlighted>> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut hasher = DefaultHasher::new();
    (lang, lines).hash(&mut hasher);
    let key = hasher.finish();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok()?.get(&key) {
        return Some(Arc::clone(hit));
    }
    let fresh = Arc::new(highlight(lang, lines)?);
    let mut cache = cache.lock().ok()?;
    if cache.len() >= CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(key, Arc::clone(&fresh));
    Some(fresh)
}

/// Highlights `lines` (one code block) as language `lang` (a fence info
/// word like `rust` or `py`). Returns styled pieces per line, or `None` if
/// the language is unknown, so the caller can use the plain code color.
pub fn highlight(lang: &str, lines: &[&str]) -> Option<Highlighted> {
    #[cfg(test)]
    CALLS.with(|c| c.set(c.get() + 1));
    let (syntaxes, theme) = assets();
    let syntax = syntax_for(syntaxes, lang)?;
    let mut h = HighlightLines::new(syntax, theme);
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        // The "newlines" syntaxes expect each line to end with '\n'.
        let with_newline = format!("{line}\n");
        let pieces = h.highlight_line(&with_newline, syntaxes).ok()?;
        out.push(
            pieces
                .into_iter()
                .map(|(style, text)| {
                    let c = style.foreground;
                    let fg = Style::default().fg(Color::Rgb(c.r, c.g, c.b));
                    (fg, text.trim_end_matches('\n').to_string())
                })
                .filter(|(_, text)| !text.is_empty())
                .collect(),
        );
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn colors(line: &[(Style, String)]) -> HashSet<Option<Color>> {
        line.iter()
            .filter(|(_, text)| !text.trim().is_empty())
            .map(|(style, _)| style.fg)
            .collect()
    }

    #[test]
    fn rust_code_gets_several_colors() {
        let out = highlight("rust", &["fn main() {", "    let s = \"hi\";", "}"]).unwrap();
        assert_eq!(out.len(), 3);
        assert!(colors(&out[0]).len() >= 2, "{:?}", out[0]);
        assert!(colors(&out[1]).len() >= 2, "{:?}", out[1]);
    }

    #[test]
    fn keyword_and_name_differ() {
        let out = highlight("rust", &["fn main() {}"]).unwrap();
        let color_of = |word: &str| {
            out[0]
                .iter()
                .find(|(_, t)| t.contains(word))
                .map(|(s, _)| s.fg)
                .unwrap()
        };
        assert_ne!(color_of("fn"), color_of("main"));
    }

    #[test]
    fn text_is_kept_exactly() {
        let src = ["def f(x):", "    return x  # comment"];
        let out = highlight("python", &src).unwrap();
        for (line, pieces) in src.iter().zip(&out) {
            let joined: String = pieces.iter().map(|(_, t)| t.as_str()).collect();
            assert_eq!(&joined, line);
        }
    }

    #[test]
    fn language_aliases_and_extensions_work() {
        assert!(highlight("py", &["x = 1"]).is_some());
        assert!(highlight("js", &["let x = 1;"]).is_some());
        assert!(
            highlight("Rust", &["fn f() {}"]).is_some(),
            "case-insensitive"
        );
    }

    #[test]
    fn cached_result_matches_a_fresh_one() {
        let src = ["fn cached() {}"];
        let first = highlight_cached("rust", &src).unwrap();
        let again = highlight_cached("rust", &src).unwrap();
        assert!(
            Arc::ptr_eq(&first, &again),
            "second call is served from the cache"
        );
        assert_eq!(*first, highlight("rust", &src).unwrap());
        assert!(highlight_cached("no-such-language", &src).is_none());
    }

    #[test]
    fn unknown_or_missing_language_is_none() {
        assert!(highlight("no-such-language", &["x"]).is_none());
        assert!(highlight("", &["x"]).is_none());
    }
}
