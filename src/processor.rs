//! Code block processors: a host (e.g. an app with plugins) can show
//! fenced code blocks of some languages as something else, like Obsidian's
//! `registerMarkdownCodeBlockProcessor` (queries, diagrams …). While the
//! cursor is in such a block, its source is shown, to edit it.

use std::path::Path;

use ratatui::text::Line;

/// Renders ```` ```lang ```` blocks of the languages it handles. Set it as
/// [`crate::shared::Shared::processor`].
/// An action on part of a rendered row: (from, to, action), by display
/// column (`to` exclusive; `usize::MAX`: to the row's end).
pub type Part = (usize, usize, String);

/// A rendered row and the actions on its parts.
pub type CellRow = (Line<'static>, Vec<Part>);

pub trait CodeBlockProcessor {
    /// Whether blocks in `lang` (the word after the opening fence) are
    /// rendered by [`CodeBlockProcessor::render`] instead of shown as code.
    fn handles(&self, lang: &str) -> bool;

    /// The lines to show for a block in `lang` whose body is `source`, in
    /// the note at `from` (`None`: untitled), for `width` columns. Longer
    /// lines are wrapped. Called for every frame that shows the block, so
    /// cache what's expensive.
    fn render(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<Line<'static>>;

    /// [`CodeBlockProcessor::render`]'s lines, each with an optional
    /// action: in view mode, Enter or a click on the row gives it to the
    /// host as [`crate::view::Outcome::Action`] (e.g. `open:Books/Dune.md`).
    /// The default has no actions.
    fn render_rows(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<(Line<'static>, Option<String>)> {
        self.render(lang, source, from, width)
            .into_iter()
            .map(|line| (line, None))
            .collect()
    }

    /// [`CodeBlockProcessor::render_rows`]'s lines with actions on parts
    /// of them: (from, to, action) by display column of the line (`to`
    /// exclusive), e.g. a calendar's days. A click on a part gives its
    /// action to the host; in view mode Tab goes from part to part. The
    /// default puts each row's action on the whole row.
    fn render_cells(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<CellRow> {
        self.render_rows(lang, source, from, width)
            .into_iter()
            .map(|(line, action)| {
                let parts = action.map(|a| vec![(0, usize::MAX, a)]);
                (line, parts.unwrap_or_default())
            })
            .collect()
    }
}
