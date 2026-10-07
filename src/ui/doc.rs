//! A document laid out for display: each line rendered (or raw, at the
//! cursor) and soft-wrapped into screen rows, with folds, tables, embeds,
//! syntax highlighting and heading sizes.

use std::cell::RefCell;

use super::*;
use crate::blocks::QuoteCode;
use crate::images::{self, ImageLink, decoded_target, image_link};
use crate::markdown::{Block, parse_line};
use crate::processor::CodeBlockProcessor;
use crate::resolver::Resolver;
use crate::search;
use crate::selection::Pos;
use crate::wrap::ImageSlot;

/// Where a document's links point: the resolver and the note's own file
/// (`None` for an untitled note).
#[derive(Clone, Copy)]
pub struct Links<'a> {
    pub resolver: &'a dyn Resolver,
    pub from: Option<&'a Path>,
}

/// A document prepared for display: its block structure, the line shown
/// raw (the cursor line), and the user's rendering options.
pub(super) struct Doc<'a> {
    lines: &'a [String],
    structure: Arc<Structure>,
    cursor: Option<usize>,
    /// The selected range (V-19): its lines are shown raw, like the cursor
    /// line, with the selected text reversed.
    selection: Option<(Pos, Pos)>,
    /// The search being typed (V-20) and the cursor's char column: its
    /// matches are highlighted, the one at the cursor more strongly.
    search: Option<(&'a str, usize)>,
    options: Options,
    /// Ctrl+K fold choices by header line (`true` = collapsed).
    folds: &'a HashMap<usize, bool>,
    /// Syntax-highlighted fenced code blocks (B-09), by their opening
    /// line; worked out when a line of the block is drawn (a long note
    /// has more blocks than the highlight cache holds).
    highlighted: RefCell<HashMap<usize, Option<Arc<Highlighted>>>>,
    /// How embeds and images find their files; `None` shows embeds as
    /// links (e.g. inside an embedded note, so embeds can't loop).
    links: Option<Links<'a>>,
    /// The size of a terminal cell in pixels, to size images (E-04);
    /// `None` when images are off.
    image_cell: Option<(u16, u16)>,
    /// The host's code block processor, if any.
    processor: Option<&'a dyn CodeBlockProcessor>,
    /// Tables the host shows its own way ([`crate::markdown::set_table_cells`]):
    /// their lines and widths, by table, worked out when drawn.
    shown_tables: RefCell<HashMap<usize, Option<Arc<ShownTable>>>>,
}

/// A table as the host shows it: its lines and column widths.
type ShownTable = (Vec<String>, Vec<usize>);

/// A fence's language: a top-level fence or one in a quote.
fn block_lang(context: &LineContext) -> Option<&str> {
    match context {
        LineContext::FenceOpen { lang } => Some(lang),
        LineContext::Quote {
            code: Some(QuoteCode::Open(lang)),
            ..
        } => Some(lang),
        _ => None,
    }
}

impl<'a> Doc<'a> {
    pub(super) fn new(
        lines: &'a [String],
        cursor: Option<usize>,
        options: Options,
        folds: &'a HashMap<usize, bool>,
        links: Option<Links<'a>>,
    ) -> Self {
        let structure = analyze_cached(lines);
        Doc {
            lines,
            structure,
            cursor,
            selection: None,
            search: None,
            options,
            folds,
            highlighted: RefCell::new(HashMap::new()),
            links,
            image_cell: Some((10, 20)),
            processor: None,
            shown_tables: RefCell::new(HashMap::new()),
        }
    }

    /// The host's lines and widths for table `t`, if it shows it its own
    /// way (not while the cursor is in it).
    fn shown_table(&self, t: usize) -> Option<Arc<ShownTable>> {
        let table = &self.structure.tables[t];
        if touches(self.cursor, self.selection, table.start, table.end) {
            return None;
        }
        self.shown_tables
            .borrow_mut()
            .entry(t)
            .or_insert_with(|| {
                let lines = &self.lines[table.start..=table.end];
                let shown = crate::markdown::host_table(lines)?;
                let widths = crate::blocks::table_widths(&shown);
                Some(Arc::new((shown, widths)))
            })
            .clone()
    }

    /// The same document with the host's code blocks rendered by
    /// `processor`.
    pub(super) fn with_processor(mut self, processor: Option<&'a dyn CodeBlockProcessor>) -> Self {
        self.processor = processor;
        self
    }

    /// The processor and the language of the processed block that line `i`
    /// is in, unless the cursor or selection is in the block.
    fn processed_block(&self, i: usize) -> Option<(&'a dyn CodeBlockProcessor, usize, usize)> {
        let processor = self.processor?;
        let (start, end) = self.structure.reveal_group(i)?;
        let lang = block_lang(&self.structure.context[start])?;
        (processor.handles(lang) && !touches(self.cursor, self.selection, start, end))
            .then_some((processor, start, end))
    }

    /// Whether line `i` opens a block the host renders (shown rendered).
    pub(super) fn opens_processed_block(&self, i: usize) -> bool {
        self.processed_block(i)
            .is_some_and(|(_, start, _)| start == i)
    }

    /// A processed block (at its opening line): the block's frame around
    /// what the processor renders.
    fn processed_rows(
        &self,
        processor: &dyn CodeBlockProcessor,
        start: usize,
        end: usize,
        width: usize,
    ) -> Wrapped {
        let context = &self.structure.context;
        let lang = block_lang(&context[start]).expect("a processed block starts with its fence");
        let dim = Style::default().fg(Color::DarkGray);
        // The body: between the fences (an unclosed block runs to the end).
        let closed = matches!(
            &context[end],
            LineContext::FenceClose
                | LineContext::Quote {
                    code: Some(QuoteCode::Close),
                    ..
                }
        );
        let body_end = if closed { end } else { end + 1 };
        // In a quote or callout: its bars before every row, the body
        // without its `>`s.
        let (bars, source): (Vec<Span<'static>>, Vec<String>) = match &context[start] {
            LineContext::Quote { callouts, code } => {
                let open = render_quote(&self.lines[start], callouts, code.as_ref()).line;
                let mut bars = open.spans;
                bars.pop(); // the frame's label
                let body = self.lines[start + 1..body_end]
                    .iter()
                    .map(|l| match parse_line(l) {
                        Block::Quote { text, .. } => text.to_string(),
                        _ => l.clone(),
                    })
                    .collect();
                (bars, body)
            }
            _ => (Vec::new(), self.lines[start + 1..body_end].to_vec()),
        };
        let bars_width: usize = bars.iter().map(|s| s.content.width()).sum();
        let with_bars = |line: Line<'static>| {
            let mut spans = bars.clone();
            spans.extend(line.spans);
            Line::from(spans)
        };
        let from = self.links.and_then(|l| l.from);
        let mut out = Wrapped::empty();
        out.push_line(with_bars(render_fence_open(lang)));
        out.actions.push(Vec::new());
        let inner = width.saturating_sub(2 + bars_width);
        let shift = 2 + bars_width;
        for (line, parts) in processor.render_cells(lang, &source, from, inner) {
            let mut spans = bars.clone();
            spans.push(Span::styled("│ ", dim));
            // The line's own style is its spans' base (a host's heading).
            let base = line.style;
            spans.extend(line.spans.into_iter().map(|s| {
                let style = base.patch(s.style);
                s.style(style)
            }));
            let rows = wrap(&Line::from(spans), 2 + bars_width, width).rows;
            for (k, row) in rows.into_iter().enumerate() {
                // The parts' columns after the frame; a part of a wrapped
                // line stays on its first row, a whole-row action on all.
                let actions = parts
                    .iter()
                    .filter(|&&(_, to, _)| to == usize::MAX || k == 0)
                    .map(|(from, to, a)| {
                        let to = if *to == usize::MAX { *to } else { to + shift };
                        (from + shift, to, a.clone())
                    })
                    .collect();
                out.push_line(row);
                out.actions.push(actions);
            }
        }
        out.push_line(with_bars(render_fence_close()));
        out.actions.push(Vec::new());
        out
    }

    /// The same document with images sized for cells of `cell` pixels, or
    /// only their titles if `None` (images off).
    pub(super) fn with_image_cell(mut self, cell: Option<(u16, u16)>) -> Self {
        self.image_cell = cell;
        self
    }

    /// An image embed line (`![[photo.png]]`, E-04 / E-05): a title row,
    /// then empty framed rows that the image is drawn over.
    fn image_rows(&self, link: &ImageLink, width: usize) -> Wrapped {
        let dim = Style::default().fg(Color::DarkGray);
        let mut out = Wrapped::empty();
        let mut title = |text: String| out.push_line(Line::from(Span::styled(text, dim)));
        let label = if link.alt.is_empty() {
            link.name()
        } else {
            &link.alt
        };
        let file = self
            .links
            .filter(|_| !link.is_remote())
            .and_then(|l| l.resolver.resolve(l.from, &decoded_target(link)).ok());
        let image = file.as_deref().and_then(images::load);
        match (&file, &image) {
            _ if link.is_remote() => title(format!(
                "╭─ 🖼 {label} · {} (web images aren't downloaded)",
                link.target
            )),
            (None, _) => title(format!("╭─ ⚠ {} (not found)", link.name())),
            (Some(_), None) => title(format!("╭─ ⚠ {} (can't show this image)", link.name())),
            (Some(path), Some(image)) => {
                let (w, h) = (image.width(), image.height());
                let at = match (link.width, link.height) {
                    (Some(pw), Some(ph)) => format!(" at {pw}×{ph} px"),
                    (Some(pw), None) => format!(" at {pw} px wide"),
                    (None, Some(ph)) => format!(" at {ph} px high"),
                    (None, None) => String::new(),
                };
                let Some(cell) = self.image_cell else {
                    title(format!("╭─ 🖼 {label} · {w}×{h}{at} (images are off)"));
                    out.push_line(Line::from(Span::styled("╰─", dim)));
                    return out;
                };
                title(format!("╭─ 🖼 {label} · {w}×{h}{at}"));
                let max_cols = u16::try_from(width.saturating_sub(2)).unwrap_or(u16::MAX);
                let (cols, rows) = images::cell_size(
                    (w, h),
                    (link.width, link.height),
                    cell,
                    if width == 0 { u16::MAX } else { max_cols },
                );
                for _ in 0..rows {
                    out.push_line(Line::from(Span::styled("│", dim)));
                }
                out.image = Some(ImageSlot {
                    path: path.clone(),
                    row: 1,
                    col: 2,
                    cols,
                    rows,
                });
            }
        }
        out.push_line(Line::from(Span::styled("╰─", dim)));
        out
    }

    /// The same document with `selection` shown.
    pub(super) fn with_selection(mut self, selection: Option<(Pos, Pos)>) -> Self {
        if selection.is_some() {
            self.selection = selection;
        }
        self
    }

    /// The same document with the matches of `query` highlighted (V-20);
    /// `cursor_col` marks the current match on the cursor line.
    pub(super) fn with_search(mut self, query: Option<&'a str>, cursor_col: usize) -> Self {
        self.search = query.filter(|q| !q.is_empty()).map(|q| (q, cursor_col));
        self
    }

    /// Highlights the search matches in line `i`'s view `r`, found in the
    /// text as shown (so a heading shown in capitals, or text with its
    /// markup hidden, still matches).
    fn show_search(&self, i: usize, r: &mut Rendered) {
        let Some((query, cursor_col)) = self.search else {
            return;
        };
        let shown: String = r.line.spans.iter().map(|s| s.content.as_ref()).collect();
        // The cursor line is raw: its text is the line with tabs widened.
        let current = (self.cursor == Some(i)).then(|| self.widen(i, cursor_col));
        for (from, to) in search::highlights(&shown, query) {
            let style = if current == Some(from) {
                CURRENT_MATCH
            } else {
                SEARCH_MATCH
            };
            restyle_chars(&mut r.line, from, to, |s| s.patch(style));
        }
    }

    /// Char column `col` of line `i` in its raw view (tabs are widened).
    fn widen(&self, i: usize, col: usize) -> usize {
        let tabs = self.lines[i]
            .chars()
            .take(col)
            .filter(|&c| c == '\t')
            .count();
        col + tabs * (TAB.len() - 1)
    }

    /// Whether line `i` is shown raw for editing: the cursor line or a
    /// selected line.
    fn active(&self, i: usize) -> bool {
        touches(self.cursor, self.selection, i, i)
    }

    /// Reverses the selected part of line `i`'s raw view `r`.
    fn show_selection(&self, i: usize, r: &mut Rendered) {
        if let Some((from, to)) = self.selected_columns(i) {
            let reversed = |s: Style| s.add_modifier(Modifier::REVERSED);
            restyle_chars(
                &mut r.line,
                self.widen(i, from),
                self.widen(i, to),
                reversed,
            );
        }
    }

    /// The selected char columns of line `i`, if any.
    fn selected_columns(&self, i: usize) -> Option<(usize, usize)> {
        let ((r0, c0), (r1, c1)) = self.selection?;
        if !(r0..=r1).contains(&i) {
            return None;
        }
        let from = if i == r0 { c0 } else { 0 };
        let to = if i == r1 {
            c1
        } else {
            self.lines[i].chars().count()
        };
        Some((from, to))
    }

    /// An embed line (`![[Note]]`, E-01 / E-02) drawn as a frame around the
    /// other note, rendered without a cursor and with its own embeds left
    /// as links.
    fn embed_rows(&self, path: &str, heading: Option<&str>, width: usize) -> Option<Wrapped> {
        let links = self.links?;
        let dim = Style::default().fg(Color::DarkGray);
        let title = match heading {
            Some(h) => format!("{path} › {h}"),
            None => path.to_string(),
        };
        let content = links
            .resolver
            .resolve(links.from, path)
            .ok()
            .and_then(|file| links.resolver.embed(&file, heading));
        let mut out = Wrapped::empty();
        // Another kind of file the host doesn't show: the line as it is.
        if content.is_none() && !crate::links::is_note_path(path) {
            return None;
        }
        let Some(content) = content else {
            out.push_line(Line::from(Span::styled(
                format!("╭─ ⚠ {title} (not found)"),
                dim,
            )));
            out.push_line(Line::from(Span::styled("╰─", dim)));
            return Some(out);
        };
        out.push_line(Line::from(Span::styled(format!("╭─ ⧉ {title}"), dim)));
        let no_folds = HashMap::new();
        // The host's code blocks render in embeds too (a base, a query).
        let inner =
            Doc::new(&content, None, self.options, &no_folds, None).with_processor(self.processor);
        for k in 0..content.len() {
            for row in inner.rows(k, width.saturating_sub(2)).rows {
                let mut spans = vec![Span::styled("│ ", dim)];
                spans.extend(row.spans);
                out.push_line(Line::from(spans));
            }
        }
        out.push_line(Line::from(Span::styled("╰─", dim)));
        Some(out)
    }

    /// Line `i` wrapped into screen rows (none if it's in a collapsed fold).
    pub(super) fn rows(&self, i: usize, width: usize) -> Wrapped {
        let (lines, structure) = (self.lines, &self.structure);
        // Source mode (V-13): every line raw; no folds, embeds or tables.
        if self.options.source_mode {
            let mut r = render_source_with(&lines[i], &self.options);
            self.show_search(i, &mut r);
            self.show_selection(i, &mut r);
            return wrap(&r.line, r.indent, width);
        }
        if structure.hiding(i, self.folds).next().is_some() {
            return Wrapped::empty();
        }
        // A line the host hides, but at the cursor.
        if self.cursor != Some(i) && crate::markdown::host_hides(&lines[i]) {
            return Wrapped::empty();
        }
        // A host's code block: its rendering on the opening line; the rest
        // of the block has no rows of its own.
        if let Some((processor, start, end)) = self.processed_block(i) {
            if i == start {
                return self.processed_rows(processor, start, end, width);
            }
            return Wrapped::empty();
        }
        // Tables (B-11): rows aren't wrapped; borders above and below.
        if let LineContext::TableRow { table, kind } = &structure.context[i]
            && structure
                .reveal_group(i)
                .is_none_or(|(a, b)| !touches(self.cursor, self.selection, a, b))
        {
            let t = &structure.tables[*table];
            let shown = self.shown_table(*table);
            let widths = shown.as_ref().map_or(&t.widths, |s| &s.1);
            let mut r = self.view(i);
            self.show_search(i, &mut r);
            let mut rows = wrap(&r.line, 0, 0);
            if *kind == TableRowKind::Header {
                rows.pad_top_with(table_border(widths, "┌", "┬", "┐"));
            }
            if i == t.end {
                rows.pad_bottom_with(table_border(widths, "└", "┴", "┘"));
            }
            return rows;
        }
        if !self.active(i)
            && structure.context[i] == LineContext::Normal
            && let Some(link) = image_link(&lines[i])
        {
            return self.image_rows(&link, width);
        }
        if !self.active(i)
            && structure.context[i] == LineContext::Normal
            && let Some(Link::File { path, heading }) = crate::links::any_embed_target(&lines[i])
            && let Some(rows) = self.embed_rows(&path, heading.as_deref(), width)
        {
            return rows;
        }
        let mut r = self.view(i);
        self.show_search(i, &mut r);
        self.show_selection(i, &mut r);
        // Fold markers: a foldable callout's header ends with ▸ (folded) or
        // ▾ (B-05); a folded section or list item says how much is hidden,
        // also while it's being edited (V-06, V-07).
        for fold in structure.folds.iter().filter(|f| f.header == i) {
            let collapsed = structure.is_collapsed(fold, self.folds);
            if fold.kind == FoldKind::Callout && !self.active(i) {
                let style = r.line.spans.last().map(|s| s.style).unwrap_or_default();
                let sign = if collapsed { " ▸" } else { " ▾" };
                r.line.spans.push(Span::styled(sign, style));
            } else if fold.kind != FoldKind::Callout && collapsed {
                let n = fold.end + 1 - fold.start;
                let lines = if n == 1 { "line" } else { "lines" };
                r.line.spans.push(Span::styled(
                    format!(" ▸ {n} {lines}"),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        // R-16: level-1/2 headings (not being edited) in double-size lines.
        let heading = structure.heading(lines, i).map(|(level, _)| level);
        let level = heading.filter(|&l| l <= 2 && self.options.heading_sizes && !self.active(i));
        let mut rows = match level {
            Some(level) => {
                let mut rows = wrap(&r.line, r.indent, width / 2);
                rows.enlarge(level);
                rows
            }
            None => wrap(&r.line, r.indent, width),
        };
        // R-17: a blank row above a heading that follows a non-blank line, in
        // the rendered and the raw view alike (so the text doesn't jump).
        if heading.is_some() && i > 0 && !lines[i - 1].trim().is_empty() {
            rows.pad_top(1);
        }
        rows
    }

    /// Fenced code line `i`'s block, highlighted, and the line's index in
    /// it; `None` for an unknown language or a block being edited.
    fn highlighted_line(&self, i: usize) -> Option<(Arc<Highlighted>, usize)> {
        let (open, _) = self.structure.reveal_group(i)?;
        let LineContext::FenceOpen { lang } = &self.structure.context[open] else {
            return None;
        };
        let block = self
            .highlighted
            .borrow_mut()
            .entry(open)
            .or_insert_with(|| {
                let (start, end) = self.structure.reveal_group(open)?;
                if touches(self.cursor, self.selection, start, end) {
                    return None;
                }
                let body: Vec<&str> = (open + 1..self.lines.len())
                    .take_while(|&k| self.structure.context[k] == LineContext::FenceBody)
                    .map(|k| self.lines[k].as_str())
                    .collect();
                highlight_cached(lang, &body)
            })
            .clone()?;
        Some((block, i - open - 1))
    }

    /// Renders line `i`: [`Doc::render_line`], with its links to missing
    /// notes dimmed (K-11) when the document knows where links point.
    fn view(&self, i: usize) -> Rendered {
        let line = &self.lines[i];
        let missing: Vec<String> = match self.links {
            Some(links) if line.contains("[[") => crate::links::all_links(line)
                .into_iter()
                .filter_map(|(link, _)| match link {
                    crate::links::Link::File { path, heading } => Some((path, heading)),
                    crate::links::Link::Web(_) => None,
                })
                .filter(|(path, _)| !path.is_empty())
                .filter(|(path, _)| !links.resolver.exists(links.from, path))
                .map(|(path, heading)| match heading {
                    Some(h) => format!("{path}#{h}"),
                    None => path,
                })
                .collect(),
            _ => Vec::new(),
        };
        if missing.is_empty() {
            return self.render_line(i);
        }
        crate::markdown::with_missing(missing, || self.render_line(i))
    }

    /// Renders line `i`. The cursor line is shown raw; so is every line of a
    /// reveal group (frontmatter, fenced code) that contains the cursor.
    fn render_line(&self, i: usize) -> Rendered {
        let (lines, structure) = (self.lines, &self.structure);
        let line = &lines[i];
        let revealed = match structure.reveal_group(i) {
            Some((start, end)) => touches(self.cursor, self.selection, start, end),
            None => self.active(i),
        };
        if revealed {
            return render_source_with(line, &self.options);
        }
        match &structure.context[i] {
            LineContext::Normal => render_with(line, &self.options),
            LineContext::Frontmatter => {
                // The key a `- item` line belongs to: the last one above it.
                let owner = lines[..i]
                    .iter()
                    .rev()
                    .find(|l| !l.starts_with([' ', '\t']) && !l.trim_start().starts_with("- "))
                    .and_then(|l| l.split_once(':'))
                    .map(|(k, _)| k.trim());
                render_frontmatter_line(line, owner).into()
            }
            LineContext::Comment => render_comment_line(line).into(),
            LineContext::FenceOpen { lang } => render_fence_open(lang).into(),
            LineContext::FenceBody => match self.highlighted_line(i) {
                Some((block, k)) if k < block.len() => render_code_pieces(&block[k]),
                _ => render_code_line(line),
            },
            LineContext::FenceClose => render_fence_close().into(),
            LineContext::Quote { callouts, code } => render_quote(line, callouts, code.as_ref()),
            LineContext::TableRow { table, kind } => {
                let t = &structure.tables[*table];
                // The host's own cells, if it shows the table its way.
                let shown = self.shown_table(*table);
                let (line, widths) = match &shown {
                    Some(s) => (s.0[i - t.start].as_str(), &s.1),
                    None => (line.as_str(), &t.widths),
                };
                match kind {
                    TableRowKind::Separator => table_border(widths, "├", "┼", "┤").into(),
                    TableRowKind::Header => render_table_row(line, widths, &t.aligns, true).into(),
                    TableRowKind::Body => render_table_row(line, widths, &t.aligns, false).into(),
                }
            }
            LineContext::IndentedCode => render_code_line(dedent_code(line)),
            LineContext::LinkDefinition => render_link_definition(line).into(),
            LineContext::ListContinuation { owner } => {
                render_continuation(line, &lines[*owner], &self.options)
            }
            LineContext::SetextHeading { level } => {
                let heading = format!("{} {}", "#".repeat(*level as usize), line.trim());
                render_with(&heading, &self.options)
            }
            LineContext::SetextUnderline { level, width } => {
                let rule = if *level == 1 { "═" } else { "─" };
                let color = heading_style_with(*level, &self.options)
                    .fg
                    .unwrap_or_default();
                Line::from(Span::styled(
                    rule.repeat(*width),
                    Style::default().fg(color),
                ))
                .into()
            }
        }
    }
}

/// Whether lines `a..=b` contain the cursor line or a selected line.
fn touches(cursor: Option<usize>, selection: Option<(Pos, Pos)>, a: usize, b: usize) -> bool {
    cursor.is_some_and(|c| (a..=b).contains(&c))
        || selection.is_some_and(|((r0, _), (r1, _))| r0 <= b && a <= r1)
}

/// Restyles the chars `from..to` of `line` with `restyle` (the selection,
/// search matches), splitting spans where needed.
fn restyle_chars(
    line: &mut Line<'static>,
    from: usize,
    to: usize,
    restyle: impl Fn(Style) -> Style,
) {
    let mut at = 0;
    let mut spans = Vec::new();
    for span in line.spans.drain(..) {
        let len = span.content.chars().count();
        let (a, b) = (from.clamp(at, at + len) - at, to.clamp(at, at + len) - at);
        at += len;
        if a == b {
            spans.push(span);
            continue;
        }
        let text: Vec<char> = span.content.chars().collect();
        let piece = |x: usize, y: usize| text[x..y].iter().collect::<String>();
        spans.push(Span::styled(piece(0, a), span.style));
        spans.push(Span::styled(piece(a, b), restyle(span.style)));
        spans.push(Span::styled(piece(b, len), span.style));
    }
    spans.retain(|s| !s.content.is_empty());
    line.spans = spans;
}

/// An indented code line without its 4 columns of indentation.
fn dedent_code(line: &str) -> &str {
    if let Some(rest) = line.strip_prefix('\t') {
        return rest;
    }
    let spaces = line.len() - line.trim_start_matches(' ').len();
    &line[spaces.min(4)..]
}
