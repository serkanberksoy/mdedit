//! Emoji picker (T-14): searches emoji by name and shortcode and keeps the
//! state of the picker popup (query, results, highlighted cell).

use std::path::PathBuf;

use emojis::Emoji;

/// Emoji per row of the picker grid (each cell is 3 columns: emoji + gap).
pub const COLUMNS: usize = 16;

/// Every emoji the picker offers, in Unicode order (by category).
///
/// Sequences that Konsole draws wider than the 2 cells they're measured as
/// are left out (R-18): joined emoji (ZWJ, e.g. 🧑‍🦳) and flags made of
/// regional-indicator letters (🇹🇷) or tag characters (🏴󠁧󠁢󠁥󠁮󠁧󠁿).
/// Found by `tests/konsole.rs`.
pub fn all() -> impl Iterator<Item = &'static Emoji> {
    emojis::iter().filter(|e| !e.as_str().chars().any(too_wide_in_terminals))
}

/// Chars that make an emoji sequence render wider than measured in Konsole.
fn too_wide_in_terminals(c: char) -> bool {
    c == '\u{200d}' // zero-width joiner
        || ('\u{1F1E6}'..='\u{1F1FF}').contains(&c) // regional indicators
        || ('\u{E0020}'..='\u{E007F}').contains(&c) // tag characters
}

/// Emoji matching `query`, best matches first (EP-02, EP-05). Ranking:
/// exact name or shortcode, then name/shortcode prefix, then the start of a
/// word, then anywhere in the text, then fuzzy (the query's letters in
/// order). Ties keep Unicode order. An empty query returns everything.
pub fn search(query: &str) -> Vec<&'static Emoji> {
    let q = query.trim().to_lowercase();
    let mut ranked: Vec<(u8, &'static Emoji)> =
        all().filter_map(|e| rank(e, &q).map(|r| (r, e))).collect();
    ranked.sort_by_key(|&(r, _)| r); // stable: ties keep Unicode order
    ranked.into_iter().map(|(_, e)| e).collect()
}

/// How well `e` matches the lowercase query `q` (lower is better), or `None`.
fn rank(e: &Emoji, q: &str) -> Option<u8> {
    if q.is_empty() {
        return Some(0);
    }
    let texts: Vec<&str> = std::iter::once(e.name()).chain(e.shortcodes()).collect();
    let word_start = |t: &str| t.split([' ', '_', '-', ':']).any(|w| w.starts_with(q));
    if texts.contains(&q) {
        Some(0)
    } else if texts.iter().any(|t| t.starts_with(q)) {
        Some(1)
    } else if texts.iter().any(|t| word_start(t)) {
        Some(2)
    } else if texts.iter().any(|t| t.contains(q)) {
        Some(3)
    } else if texts.iter().any(|t| is_subsequence(q, t)) {
        Some(4)
    } else {
        None
    }
}

/// Whether the non-space chars of `needle` appear in `hay` in order.
fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut hay = hay.chars();
    needle
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|c| hay.any(|h| h == c))
}

/// How many recently used emoji are remembered (two grid rows).
pub const MAX_RECENT: usize = 2 * COLUMNS;

/// Recently inserted emoji, most recent first (EP-06). Remembered between
/// sessions in a small file, one emoji per line.
#[derive(Debug, Default)]
pub struct Recent {
    pub items: Vec<&'static Emoji>,
    /// Where `save` writes; `None` keeps the list in memory only (tests).
    pub path: Option<PathBuf>,
}

impl Recent {
    /// Reads the list from `path`. A missing or unreadable file gives an
    /// empty list; lines that aren't emoji the picker offers are skipped.
    pub fn load(path: PathBuf) -> Self {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let items = text
            .lines()
            .filter_map(|line| emojis::get(line.trim()))
            .filter(|e| all().any(|offered| offered == *e))
            .take(MAX_RECENT)
            .collect();
        Recent {
            items,
            path: Some(path),
        }
    }

    /// Moves `emoji` to the front (no duplicates, at most [`MAX_RECENT`]).
    pub fn add(&mut self, emoji: &'static Emoji) {
        self.items.retain(|e| *e != emoji);
        self.items.insert(0, emoji);
        self.items.truncate(MAX_RECENT);
    }

    /// Writes the list to its file, creating the folder if needed.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text: String = self
            .items
            .iter()
            .map(|e| format!("{}\n", e.as_str()))
            .collect();
        std::fs::write(path, text)
    }
}

/// `$XDG_CONFIG_HOME/mdedit/recent_emoji`, or `~/.config/mdedit/recent_emoji`
/// (`%APPDATA%\mdedit\recent_emoji` on Windows).
pub fn default_recent_path() -> Option<PathBuf> {
    Some(
        crate::platform::config_home()?
            .join("mdedit")
            .join("recent_emoji"),
    )
}

/// Picker popup state.
#[derive(Debug, Default)]
pub struct Picker {
    pub query: String,
    pub results: Vec<&'static Emoji>,
    /// Index into `results` of the highlighted emoji.
    pub selected: usize,
    /// Shown first when the search is empty (EP-06).
    pub recent: Vec<&'static Emoji>,
}

impl Picker {
    pub fn new() -> Self {
        Self::with_recent(&[])
    }

    /// A picker whose empty-search list starts with `recent` (EP-06).
    pub fn with_recent(recent: &[&'static Emoji]) -> Self {
        let mut p = Picker {
            recent: recent.to_vec(),
            ..Picker::default()
        };
        p.set_query(String::new());
        p
    }

    /// Replaces the query and re-filters; the highlight goes to the first
    /// result. An empty query lists the recent emoji first, then the rest.
    pub fn set_query(&mut self, query: String) {
        self.results = if query.trim().is_empty() {
            let rest = all().filter(|e| !self.recent.contains(e));
            self.recent.iter().copied().chain(rest).collect()
        } else {
            search(&query)
        };
        self.query = query;
        self.selected = 0;
    }

    /// Moves the highlight by `dx` cells and `dy` rows of the grid, staying
    /// inside the results.
    pub fn move_by(&mut self, dx: isize, dy: isize) {
        let Some(last) = self.results.len().checked_sub(1) else {
            return;
        };
        let target = self.selected as isize + dx + dy * COLUMNS as isize;
        self.selected = target.clamp(0, last as isize) as usize;
    }

    pub fn current(&self) -> Option<&'static Emoji> {
        self.results.get(self.selected).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(query: &str) -> Vec<&'static str> {
        search(query).iter().map(|e| e.as_str()).collect()
    }

    fn rank(query: &str, emoji: &str) -> usize {
        found(query)
            .iter()
            .position(|e| *e == emoji)
            .unwrap_or_else(|| panic!("{emoji} not found for {query:?}"))
    }

    #[test]
    fn empty_query_lists_everything_in_unicode_order() {
        let all_emoji = found("");
        assert_eq!(all_emoji.len(), all().count());
        assert_eq!(all_emoji[0], "😀");
    }

    #[test]
    fn joined_sequences_are_left_out() {
        assert!(all().all(|e| !e.as_str().contains('\u{200d}')));
        assert!(all().count() > 1000);
    }

    #[test]
    fn flag_sequences_are_left_out() {
        let regional = |c: char| ('\u{1F1E6}'..='\u{1F1FF}').contains(&c);
        let tag = |c: char| ('\u{E0020}'..='\u{E007F}').contains(&c);
        assert!(all().all(|e| !e.as_str().chars().any(|c| regional(c) || tag(c))));
        assert!(found("flag").contains(&"🏁"), "single-glyph flags stay");
    }

    #[test]
    fn name_prefix_comes_first() {
        assert_eq!(found("grinning")[0], "😀");
    }

    #[test]
    fn search_is_case_insensitive() {
        assert_eq!(found("GRINNING")[0], "😀");
    }

    #[test]
    fn exact_name_or_shortcode_beats_prefix() {
        // 🐱 has the shortcode `cat`, 🐈 the name "cat"; "cat face" etc. follow.
        let cats = found("cat");
        assert_eq!(cats[..2], ["🐱", "🐈"]);
    }

    #[test]
    fn shortcodes_are_searched() {
        assert_eq!(found("thumbsup")[0], "👍");
        assert!(found("+1").contains(&"👍"));
    }

    #[test]
    fn word_start_beats_middle_of_word() {
        // "thumbs up" has a word starting with "up"; "cupcake" only contains it.
        assert!(rank("up", "👍") < rank("up", "🧁"));
    }

    #[test]
    fn fuzzy_matches_letters_in_order() {
        assert!(found("grnng fc").contains(&"😀"));
        assert!(rank("grinning", "😀") < 5);
        assert!(found("zzqxj").is_empty());
    }

    #[test]
    fn fuzzy_comes_after_substring_matches() {
        let contains =
            |e: &Emoji| e.name().contains("cake") || e.shortcodes().any(|s| s.contains("cake"));
        let results = search("cake");
        let first_fuzzy = results.iter().position(|e| !contains(e)).unwrap();
        assert!(first_fuzzy > 0, "substring matches exist");
        assert!(
            results[first_fuzzy..].iter().all(|e| !contains(e)),
            "no substring match after the first fuzzy-only one"
        );
    }

    fn e(s: &str) -> &'static Emoji {
        emojis::get(s).unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("mdedit-emoji-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn recent_add_moves_to_front_without_duplicates() {
        let mut r = Recent::default();
        r.add(e("😀"));
        r.add(e("🐱"));
        r.add(e("😀"));
        let items: Vec<_> = r.items.iter().map(|x| x.as_str()).collect();
        assert_eq!(items, ["😀", "🐱"]);
    }

    #[test]
    fn recent_is_capped() {
        let mut r = Recent::default();
        for emoji in all().take(MAX_RECENT + 5) {
            r.add(emoji);
        }
        assert_eq!(r.items.len(), MAX_RECENT);
        assert_eq!(
            r.items[0],
            all().nth(MAX_RECENT + 4).unwrap(),
            "newest first"
        );
    }

    #[test]
    fn recent_survives_save_and_load() {
        let path = scratch("roundtrip").join("sub").join("recent_emoji");
        let mut r = Recent {
            path: Some(path.clone()),
            ..Recent::default()
        };
        r.add(e("🐱"));
        r.add(e("👍"));
        r.save().unwrap();
        let loaded = Recent::load(path);
        let items: Vec<_> = loaded.items.iter().map(|x| x.as_str()).collect();
        assert_eq!(items, ["👍", "🐱"]);
    }

    #[test]
    fn load_skips_junk_and_missing_file_is_empty() {
        let dir = scratch("junk");
        assert!(Recent::load(dir.join("nope")).items.is_empty());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("recent_emoji");
        std::fs::write(&path, "😀\nnot an emoji\n🇹🇷\n\n🐱\n").unwrap();
        let items: Vec<_> = Recent::load(path)
            .items
            .iter()
            .map(|x| x.as_str())
            .collect();
        assert_eq!(items, ["😀", "🐱"], "flags aren't offered by the picker");
    }

    #[test]
    fn default_path_is_in_the_config_folder() {
        let p = default_recent_path().unwrap();
        assert!(p.ends_with("mdedit/recent_emoji"), "{}", p.display());
    }

    #[test]
    fn empty_search_shows_recent_first_without_duplicates() {
        let p = Picker::with_recent(&[e("🐱"), e("👍")]);
        let first: Vec<_> = p.results[..3].iter().map(|x| x.as_str()).collect();
        assert_eq!(first, ["🐱", "👍", "😀"]);
        assert_eq!(p.results.len(), all().count(), "each emoji once");
    }

    #[test]
    fn typed_search_ignores_recent_order() {
        let mut p = Picker::with_recent(&[e("🐈")]);
        p.set_query("cat".into());
        assert_eq!(p.current().map(|x| x.as_str()), Some("🐱"));
        p.set_query(String::new());
        assert_eq!(
            p.current().map(|x| x.as_str()),
            Some("🐈"),
            "back to recent first"
        );
    }

    #[test]
    fn picker_starts_with_everything_and_first_highlighted() {
        let p = Picker::new();
        assert_eq!(p.results.len(), all().count());
        assert_eq!(p.current().map(|e| e.as_str()), Some("😀"));
    }

    #[test]
    fn new_query_resets_highlight() {
        let mut p = Picker::new();
        p.move_by(3, 1);
        p.set_query("cat".into());
        assert_eq!(p.selected, 0);
        assert_eq!(p.current().map(|e| e.as_str()), Some("🐱"));
    }

    #[test]
    fn arrows_move_in_the_grid_and_stay_inside() {
        let mut p = Picker::new();
        p.move_by(1, 0);
        assert_eq!(p.selected, 1);
        p.move_by(0, 1);
        assert_eq!(p.selected, 1 + COLUMNS);
        p.move_by(-1, -1);
        assert_eq!(p.selected, 0);
        p.move_by(-1, 0);
        assert_eq!(p.selected, 0, "no wrap before the first");
        p.move_by(0, -1);
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn down_on_last_partial_row_goes_to_last_result() {
        let mut p = Picker::new();
        p.set_query("grinning".into()); // a handful of results, one row
        let last = p.results.len() - 1;
        p.move_by(0, 1);
        assert_eq!(p.selected, last);
        p.move_by(1, 0);
        assert_eq!(p.selected, last, "no wrap after the last");
    }

    #[test]
    fn no_results_means_no_current() {
        let mut p = Picker::new();
        p.set_query("zzqxj".into());
        assert_eq!(p.current(), None);
        p.move_by(1, 1);
        assert_eq!(p.selected, 0);
    }
}
