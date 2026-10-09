//! Line-level Markdown parsing and rendering.
//!
//! Supported blocks: ATX headings (`#` .. `######`), bullet lists (`-`, `*`,
//! `+`, indented with spaces or tabs), tasks (`- [ ]`, `- [x]` and Obsidian
//! alternate states like `- [.]`), block quotes / callouts (`>`), horizontal
//! rules (`---`), plus rendering for frontmatter, fenced code and callout
//! bodies (detected document-wide in `blocks`). Inline: `[[wiki links]]`, URLs,
//! `#tags`, `**bold**` and `*italic*`. Everything else is a plain paragraph.
//!
//! This file has the line parser ([`parse_line`]) and the options; the
//! inline parser is in `inline`, line rendering in `render` and tables in
//! `table`.

use ratatui::style::{Color, Modifier, Style};

use crate::blocks::QuoteCode;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

pub(crate) mod inline;
pub mod math;
mod render;
mod table;

pub use inline::*;
pub use render::*;
pub use table::*;

/// Code text color: code spans and code blocks.
const CODE: Color = Color::LightYellow;

/// Display width of a tab in the raw (cursor-line) view.
pub const TAB: &str = "    ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block<'a> {
    /// `level` is 1..=6, `text` is the heading content without the `#` marker.
    Heading {
        level: u8,
        text: &'a str,
    },
    /// `indent` is the byte length of the leading spaces/tabs.
    Bullet {
        indent: usize,
        marker: &'a str,
        text: &'a str,
    },
    /// A bullet starting with a checkbox: `- [ ] todo`, `- [x] done`, `- [.] log`.
    /// `state` is the char between the brackets.
    Task {
        indent: usize,
        marker: &'a str,
        state: char,
        text: &'a str,
    },
    /// `> text`, `> > text` …; `depth` is the number of `>` markers and
    /// `text` excludes them.
    Quote {
        depth: usize,
        text: &'a str,
    },
    /// `---`, `***` or `___` (3 or more).
    Rule,
    Paragraph(&'a str),
}

pub fn parse_line(line: &str) -> Block<'_> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) {
        let rest = &line[hashes..];
        if rest.is_empty() || rest.starts_with(' ') {
            return Block::Heading {
                level: hashes as u8,
                text: rest.trim(),
            };
        }
    }

    let trimmed = line.trim_end();
    if trimmed.len() >= 3
        && ['-', '*', '_']
            .iter()
            .any(|&c| trimmed.chars().all(|x| x == c))
    {
        return Block::Rule;
    }

    let indent = line.chars().take_while(|&c| c == ' ' || c == '\t').count();
    let rest = &line[indent..];
    let mut chars = rest.chars();
    if let (Some('-' | '*' | '+'), Some(' ')) = (chars.next(), chars.next()) {
        return list_item(indent, &rest[..1], &rest[2..]);
    }
    // Ordered: 1–9 digits, then `.` or `)`, then a space or the end.
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if (1..=9).contains(&digits) && matches!(rest[digits..].chars().next(), Some('.' | ')')) {
        let after = &rest[digits + 1..];
        if after.is_empty() || after.starts_with(' ') {
            let text = after.strip_prefix(' ').unwrap_or(after);
            return list_item(indent, &rest[..digits + 1], text);
        }
    }

    if let Some((depth, prefix)) = quote_prefix(line) {
        let q = &line[prefix..];
        return Block::Quote {
            depth,
            text: q.strip_prefix(' ').unwrap_or(q),
        };
    }

    Block::Paragraph(line)
}

/// The quote depth of `line` and the byte length of its `>` markers
/// (`> > x` → depth 2, prefix `> >`), if it's a quote. Markers may be
/// separated by single spaces.
pub fn quote_prefix(line: &str) -> Option<(usize, usize)> {
    let b = line.as_bytes();
    let (mut depth, mut i) = (0, 0);
    while b.get(i) == Some(&b'>') {
        depth += 1;
        i += 1;
        if b.get(i) == Some(&b' ') && b.get(i + 1) == Some(&b'>') {
            i += 1;
        }
    }
    (depth > 0).then_some((depth, i))
}

/// A bullet or ordered item; a task if `text` starts with a checkbox.
fn list_item<'a>(indent: usize, marker: &'a str, text: &'a str) -> Block<'a> {
    if let Some(inner) = text.strip_prefix('[') {
        let mut it = inner.chars();
        if let (Some(state), Some(']')) = (it.next(), it.next())
            && state != ']'
            && state != '['
        {
            let after = &inner[state.len_utf8() + 1..];
            if after.is_empty() || after.starts_with(' ') {
                let text = after.strip_prefix(' ').unwrap_or(after);
                return Block::Task {
                    indent,
                    marker,
                    state,
                    text,
                };
            }
        }
    }
    Block::Bullet {
        indent,
        marker,
        text,
    }
}

/// Whether a list marker is ordered (`1.`, `2)`) rather than a bullet.
pub fn is_ordered(marker: &str) -> bool {
    marker.ends_with(['.', ')'])
}

/// The marker for the item after one with `marker`: the next number for an
/// ordered list (`9.` → `10.`), the same bullet otherwise.
pub fn next_marker(marker: &str) -> String {
    let (number, delim) = marker.split_at(marker.len() - 1);
    match number.parse::<u64>() {
        Ok(n) if is_ordered(marker) => format!("{}{delim}", n + 1),
        _ => marker.to_string(),
    }
}

/// How done tasks (`[x]`) look (R-07).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DoneStyle {
    /// Greyed out and struck through (the default).
    #[default]
    Strike,
    /// Greyed out only, as in Obsidian's default theme.
    Grey,
}

/// Spaces per list level, unless the settings say otherwise.
pub const DEFAULT_INDENT: usize = 2;

/// Rendering choices that come from the user's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub done_style: DoneStyle,
    /// Level-1/2 headings use double-size lines (R-16).
    pub heading_sizes: bool,
    /// Color per heading level (1–6) from the settings; `None` = default.
    pub heading_colors: [Option<Color>; 6],
    /// Source mode (V-13): every line raw, nothing folded or expanded.
    pub source_mode: bool,
    /// Spaces per list level (`indent_width`), for the indent guides.
    pub indent_width: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            done_style: DoneStyle::default(),
            heading_sizes: false,
            heading_colors: [None; 6],
            source_mode: false,
            indent_width: DEFAULT_INDENT,
        }
    }
}

pub fn is_done(state: char) -> bool {
    matches!(state, 'x' | 'X')
}

/// Nesting level of a list item: one tab or `width` spaces per level.
pub fn nesting(whitespace: &str, width: usize) -> usize {
    let tabs = whitespace.chars().filter(|&c| c == '\t').count();
    let spaces = whitespace.chars().filter(|&c| c == ' ').count();
    tabs + spaces / width.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hosts_verbatim_spans_are_shown_as_written() {
        let text = "a <% \"*x*\" + tp.file.title %> *b* <%* y %>";
        set_verbatim(&[("<%", "%>")]);
        let spans = inline_spans(text, Style::default());
        set_verbatim(&[]);
        assert_eq!(
            plain(&Line::from(spans.clone())),
            "a <% \"*x*\" + tp.file.title %> b <%* y %>",
            "nothing inside is Markdown; the rest is"
        );
        let tag = spans
            .iter()
            .find(|s| s.content.starts_with("<% "))
            .expect("one span");
        assert_eq!(tag.style.fg, Some(CODE));
        let without = plain(&Line::from(inline_spans(text, Style::default())));
        assert_eq!(
            without, "a <% \"x\" + tp.file.title %> b <%* y %>",
            "off again"
        );
    }

    #[test]
    fn a_hosts_marks_restyle_text_as_written() {
        use std::rc::Rc;
        let chip = Style::new().fg(Color::DarkGray).bg(Color::Black);
        set_marks(Some(Rc::new(move |t: &str| {
            t.find("📅")
                .map(|at| vec![(at..t.len(), chip)])
                .unwrap_or_default()
        })));
        let spans = inline_spans("do **it** 📅 2026-10-12", Style::default());
        let nested = inline_spans("[[A|see 📅 x]]", Style::default());
        set_marks(None);
        assert_eq!(plain(&Line::from(spans.clone())), "do it 📅 2026-10-12");
        let marked = spans.iter().find(|s| s.content.contains('📅')).unwrap();
        assert_eq!(marked.content, "📅 2026-10-12");
        assert_eq!(marked.style.bg, Some(Color::Black));
        let bold = spans.iter().find(|s| s.content == "it").unwrap();
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
        assert!(
            nested.iter().all(|s| s.style.bg != Some(Color::Black)),
            "inside markup, as it was: {nested:?}"
        );
        let off = inline_spans("x 📅 1", Style::default());
        assert!(off.iter().all(|s| s.style.bg.is_none()), "off again");
    }

    #[test]
    fn a_hosts_link_badges_and_rendered_spans() {
        use std::rc::Rc;
        set_link_badge(Some(Rc::new(|t: &str| (t == "A").then(|| "3".to_string()))));
        let spans = inline_spans("see [[A]] and [[B|b]]", Style::default());
        set_link_badge(None);
        assert_eq!(plain(&Line::from(spans.clone())), "see A 3 and b");
        let badge = spans.iter().find(|s| s.content == " 3").expect("a badge");
        assert_eq!(badge.style.fg, Some(Color::DarkGray));
        assert_eq!(
            plain(&Line::from(inline_spans("[[A]]", Style::default()))),
            "A",
            "off again"
        );
        set_rendered(vec![(
            "[@".into(),
            "]".into(),
            Rc::new(|k: &str| (k == "doe").then(|| "Doe 2020".to_string())),
        )]);
        let spans = inline_spans("cite [@doe] and [@nope] *x*", Style::default());
        set_rendered(Vec::new());
        assert_eq!(
            plain(&Line::from(spans.clone())),
            "cite Doe 2020 and [@nope] x"
        );
        let shown = spans
            .iter()
            .find(|s| s.content == "Doe 2020")
            .expect("rendered");
        assert_eq!(shown.style.fg, Some(Color::LightBlue), "a link's color");
    }

    fn plain(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn parses_heading_levels() {
        for level in 1..=6u8 {
            let src = format!("{} Title", "#".repeat(level as usize));
            assert_eq!(
                parse_line(&src),
                Block::Heading {
                    level,
                    text: "Title"
                }
            );
        }
    }

    #[test]
    fn rejects_non_headings() {
        assert_eq!(parse_line("#Title"), Block::Paragraph("#Title"));
        assert_eq!(
            parse_line("####### seven"),
            Block::Paragraph("####### seven")
        );
    }

    #[test]
    fn empty_heading_is_heading() {
        assert_eq!(parse_line("##"), Block::Heading { level: 2, text: "" });
    }

    #[test]
    fn parses_bullets_with_indent() {
        assert_eq!(
            parse_line("- item"),
            Block::Bullet {
                indent: 0,
                marker: "-",
                text: "item"
            }
        );
        assert_eq!(
            parse_line("  * nested"),
            Block::Bullet {
                indent: 2,
                marker: "*",
                text: "nested"
            }
        );
        assert_eq!(
            parse_line("\t\t- tabbed"),
            Block::Bullet {
                indent: 2,
                marker: "-",
                text: "tabbed"
            }
        );
        assert_eq!(
            parse_line("+ "),
            Block::Bullet {
                indent: 0,
                marker: "+",
                text: ""
            }
        );
        assert_eq!(parse_line("-item"), Block::Paragraph("-item"));
        assert_eq!(parse_line("-"), Block::Paragraph("-"));
        assert_eq!(
            parse_line("- [[link]]"),
            Block::Bullet {
                indent: 0,
                marker: "-",
                text: "[[link]]"
            }
        );
    }

    #[test]
    fn parses_ordered_items() {
        let o = |indent, marker, text| Block::Bullet {
            indent,
            marker,
            text,
        };
        assert_eq!(parse_line("1. first"), o(0, "1.", "first"));
        assert_eq!(parse_line("12) twelve"), o(0, "12)", "twelve"));
        assert_eq!(parse_line("\t3. nested"), o(1, "3.", "nested"));
        assert_eq!(parse_line("4. "), o(0, "4.", ""));
        assert_eq!(
            parse_line("1.5 is a number"),
            Block::Paragraph("1.5 is a number")
        );
        assert_eq!(parse_line("2024. was a year"), o(0, "2024.", "was a year"));
        assert_eq!(
            parse_line("1234567890. too long"),
            Block::Paragraph("1234567890. too long")
        );
    }

    #[test]
    fn ordered_items_show_their_number() {
        assert_eq!(plain(&render_line("1. first")), "1. first");
        assert_eq!(plain(&render_line("\t2) nested")), "│ 2) nested");
        assert_eq!(plain(&render_source_line("10. x")), "10. x");
    }

    #[test]
    fn ordered_tasks_show_number_and_checkbox() {
        assert_eq!(
            parse_line("3. [x] done"),
            Block::Task {
                indent: 0,
                marker: "3.",
                state: 'x',
                text: "done"
            }
        );
        assert_eq!(plain(&render_line("3. [x] done")), "3. ☑ done");
        assert_eq!(plain(&render_source_line("3. [x] done")), "3. [x] done");
    }

    #[test]
    fn parses_tasks() {
        let t = |indent, state, text| Block::Task {
            indent,
            marker: "-",
            state,
            text,
        };
        assert_eq!(parse_line("- [ ] todo"), t(0, ' ', "todo"));
        assert_eq!(parse_line("  - [x] done"), t(2, 'x', "done"));
        assert_eq!(parse_line("- [X] done"), t(0, 'X', "done"));
        assert_eq!(parse_line("\t- [.] log"), t(1, '.', "log"));
        assert_eq!(parse_line("- [ ]"), t(0, ' ', ""));
        assert_eq!(parse_line("- [ ] "), t(0, ' ', ""));
        assert_eq!(
            parse_line("- [x]y"),
            Block::Bullet {
                indent: 0,
                marker: "-",
                text: "[x]y"
            }
        );
        assert_eq!(
            parse_line("[ ] no bullet"),
            Block::Paragraph("[ ] no bullet")
        );
    }

    #[test]
    fn parses_rules_and_quotes() {
        assert_eq!(parse_line("---"), Block::Rule);
        assert_eq!(parse_line("----"), Block::Rule);
        assert_eq!(parse_line("***"), Block::Rule);
        assert_eq!(parse_line("--"), Block::Paragraph("--"));
        assert_eq!(
            parse_line("> hi"),
            Block::Quote {
                depth: 1,
                text: "hi"
            }
        );
        assert_eq!(parse_line(">"), Block::Quote { depth: 1, text: "" });
    }

    #[test]
    fn nested_quotes_count_their_markers() {
        let q = |depth, text| Block::Quote { depth, text };
        assert_eq!(parse_line("> > two"), q(2, "two"));
        assert_eq!(parse_line(">>two"), q(2, "two"));
        assert_eq!(parse_line("> > > three"), q(3, "three"));
        assert_eq!(parse_line("> > "), q(2, ""));
        assert_eq!(parse_line("> a > b"), q(1, "a > b"));
    }

    #[test]
    fn nested_quotes_draw_a_bar_per_level() {
        assert_eq!(plain(&render_line("> > two")), "┃ ┃ two");
        assert_eq!(render("> > two").indent, 4);
        assert_eq!(plain(&render_source_line("> > two")), "> > two");
        assert_eq!(render_source("> > two").indent, 4);
    }

    #[test]
    fn nesting_levels() {
        assert_eq!(nesting("", 2), 0);
        assert_eq!(nesting("  ", 2), 1);
        assert_eq!(nesting("\t", 2), 1);
        assert_eq!(nesting("\t\t", 2), 2);
        assert_eq!(nesting("    ", 2), 2);
        assert_eq!(nesting("    ", 4), 1, "4-space indentation");
        assert_eq!(Options::default().indent_width, 2);
        // The indent guides follow it.
        let guides = |options: &Options| {
            plain(&render_with("    - a", options).line)
                .matches('│')
                .count()
        };
        assert_eq!(guides(&Options::default()), 2);
        let four = Options {
            indent_width: 4,
            ..Options::default()
        };
        assert_eq!(guides(&four), 1);
    }

    #[test]
    fn grey_done_style_drops_the_strikethrough() {
        let grey = Options {
            done_style: DoneStyle::Grey,
            ..Options::default()
        };
        let done = render_with("- [x] a", &grey).line;
        let text = done.spans.last().unwrap();
        assert_eq!(text.content, "a");
        assert_eq!(text.style.fg, Some(Color::DarkGray));
        assert!(!text.style.add_modifier.contains(Modifier::CROSSED_OUT));
        let raw = render_source_with("- [x] a", &grey).line;
        assert!(
            !raw.spans[1]
                .style
                .add_modifier
                .contains(Modifier::CROSSED_OUT)
        );
        // Cancelled tasks stay struck through either way.
        let cancelled = render_with("- [-] a", &grey).line;
        let text = cancelled.spans.last().unwrap();
        assert!(text.style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn heading_colors_can_be_configured() {
        let mut options = Options::default();
        options.heading_colors[0] = Some(Color::Red);
        let rendered = render_with("# Title", &options).line;
        assert!(
            rendered
                .spans
                .iter()
                .all(|s| s.style.fg == Some(Color::Red))
        );
        assert!(
            rendered.spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD),
            "only the color changes"
        );
        let raw = render_source_with("# Title", &options).line;
        assert_eq!(
            raw.spans[1].style.fg,
            Some(Color::Red),
            "also while editing"
        );
        let other = render_with("## Other", &options).line;
        assert_eq!(
            other.spans[0].style.fg,
            Some(Color::Cyan),
            "unset levels keep their color"
        );
    }

    #[test]
    fn done_task_is_struck_through() {
        let open = render_line("- [ ] a");
        let done = render_line("- [x] a");
        assert_eq!(plain(&open), "☐ a");
        assert_eq!(plain(&done), "☑ a");
        assert!(
            !open.spans[2]
                .style
                .add_modifier
                .contains(Modifier::CROSSED_OUT)
        );
        assert!(
            done.spans[2]
                .style
                .add_modifier
                .contains(Modifier::CROSSED_OUT)
        );
    }

    #[test]
    fn alternate_task_states() {
        assert_eq!(plain(&render_line("\t- [.] log")), "│ ⦿ log");
        assert_eq!(plain(&render_line("- [-] nope")), "☒ nope");
        assert_eq!(plain(&render_line("- [/] wip")), "◐ wip");
        assert_eq!(plain(&render_line("- [~] odd")), "[~] odd");
    }

    #[test]
    fn inline_markup() {
        let p = |s: &str| plain(&render_line(s));
        assert_eq!(p("see [[README]] now"), "see README now");
        assert_eq!(
            p("<< [[Journal/2026-06-28|yesterday]] >>"),
            "<< yesterday >>"
        );
        assert_eq!(p("x #type/project y #on/AI"), "x #type/project y #on/AI");
        assert_eq!(p("a **bold** and *it* b"), "a bold and it b");
        assert_eq!(p("2 * 3 * 4"), "2 * 3 * 4");
        assert_eq!(p("issue #12"), "issue #12");

        let spans = inline_spans("go https://x.io/a?b now", Style::default());
        assert_eq!(spans[1].content, "https://x.io/a?b");
        assert!(spans[1].style.add_modifier.contains(Modifier::UNDERLINED));

        let spans = inline_spans("x #tag", Style::default());
        assert_eq!(spans[1].style.bg, Some(Color::Rgb(25, 55, 100)));
    }

    /// Inline spans as `(text, style)` pairs, for exact style checks.
    fn styled(text: &str) -> Vec<(String, Style)> {
        inline_spans(text, Style::default())
            .into_iter()
            .map(|s| (s.content.to_string(), s.style))
            .collect()
    }

    fn st(m: Modifier) -> Style {
        Style::default().add_modifier(m)
    }

    #[test]
    fn strikethrough() {
        assert_eq!(
            styled("a ~~b c~~ d"),
            [
                ("a ".into(), Style::default()),
                ("b c".into(), st(Modifier::CROSSED_OUT)),
                (" d".into(), Style::default()),
            ]
        );
        assert_eq!(styled("a ~b~ c"), [("a ~b~ c".into(), Style::default())]);
        assert_eq!(
            styled("~~ not ~~"),
            [("~~ not ~~".into(), Style::default())]
        );
    }

    #[test]
    fn highlight() {
        let hl = Style::default().fg(Color::Black).bg(Color::Yellow);
        assert_eq!(
            styled("a ==b== c"),
            [
                ("a ".into(), Style::default()),
                ("b".into(), hl),
                (" c".into(), Style::default()),
            ]
        );
        assert_eq!(
            styled("if a == b"),
            [("if a == b".into(), Style::default())]
        );
    }

    #[test]
    fn highlight_colors() {
        // A color emoji first colors it (T-07a); the emoji isn't shown.
        let colored = |c: Color| Style::default().fg(Color::Black).bg(c);
        assert_eq!(
            styled("a ==🔴red== ==🟣purple **bold**== b"),
            [
                ("a ".into(), Style::default()),
                ("red".into(), colored(Color::Red)),
                (" ".into(), Style::default()),
                ("purple ".into(), colored(Color::Magenta)),
                (
                    "bold".into(),
                    colored(Color::Magenta).add_modifier(Modifier::BOLD)
                ),
                (" b".into(), Style::default()),
            ]
        );
        assert_eq!(
            HIGHLIGHT_COLORS.map(|(e, c, _)| (e, c)),
            [
                ("🔴", Color::Red),
                ("🟠", Color::LightRed),
                ("🟡", Color::Yellow),
                ("🟢", Color::Green),
                ("🔵", Color::Blue),
                ("🟣", Color::Magenta),
            ]
        );
        assert_eq!(
            styled("==🔴=="),
            [("🔴".into(), colored(Color::Yellow))],
            "only the emoji: a highlight of it"
        );
    }

    #[test]
    fn inline_code_is_literal() {
        let code = Style::default().fg(CODE);
        assert_eq!(
            styled("run `a **b**` now"),
            [
                ("run ".into(), Style::default()),
                ("a **b**".into(), code),
                (" now".into(), Style::default()),
            ]
        );
        assert_eq!(styled("``a ` b``"), [("a ` b".into(), code)]);
        assert_eq!(
            styled("`` `x` ``"),
            [("`x`".into(), code)],
            "one space trimmed"
        );
        assert_eq!(
            styled("no `close"),
            [("no `close".into(), Style::default())]
        );
    }

    #[test]
    fn inline_code_with_multibyte_characters() {
        // Regression: this panicked (slicing inside '▸') and crashed mdedit.
        let code = Style::default().fg(CODE);
        assert_eq!(
            styled("ends with `▸ N lines`. ok"),
            [
                ("ends with ".into(), Style::default()),
                ("▸ N lines".into(), code),
                (". ok".into(), Style::default()),
            ]
        );
        assert_eq!(styled("`é` and `😀`")[0], ("é".into(), code));
        assert_eq!(
            styled("no ` close é ▸"),
            [("no ` close é ▸".into(), Style::default())]
        );
    }

    #[test]
    fn inline_code_keeps_outer_style() {
        assert_eq!(
            styled("**a `b`**"),
            [
                ("a ".into(), st(Modifier::BOLD)),
                ("b".into(), st(Modifier::BOLD).fg(CODE)),
            ]
        );
    }

    #[test]
    fn backslash_escapes_punctuation() {
        let plain = |t: &str| vec![(t.to_string(), Style::default())];
        assert_eq!(styled(r"\*a\*"), plain("*a*"));
        assert_eq!(styled(r"\#tag \`x\`"), plain("#tag `x`"));
        assert_eq!(
            styled(r"a\b and a\"),
            plain(r"a\b and a\"),
            "only punctuation"
        );
        assert_eq!(
            styled(r"**a\*b**"),
            [("a*b".into(), st(Modifier::BOLD))],
            "an escaped marker doesn't close"
        );
    }

    #[test]
    fn double_underscore_is_bold_but_not_inside_words() {
        assert_eq!(
            styled("a __b__ c"),
            [
                ("a ".into(), Style::default()),
                ("b".into(), st(Modifier::BOLD)),
                (" c".into(), Style::default()),
            ]
        );
        assert_eq!(styled("x__y__z"), [("x__y__z".into(), Style::default())]);
    }

    #[test]
    fn single_underscore_is_italic_but_not_inside_words() {
        assert_eq!(
            styled("an _it_."),
            [
                ("an ".into(), Style::default()),
                ("it".into(), st(Modifier::ITALIC)),
                (".".into(), Style::default()),
            ]
        );
        assert_eq!(
            styled("snake_case_words"),
            [("snake_case_words".into(), Style::default())]
        );
    }

    #[test]
    fn triple_markers_are_bold_italic() {
        let both = st(Modifier::BOLD | Modifier::ITALIC);
        assert_eq!(styled("***a***"), [("a".into(), both)]);
        assert_eq!(styled("___a___"), [("a".into(), both)]);
        assert_eq!(
            styled("**a *b* c**"),
            [
                ("a ".into(), st(Modifier::BOLD)),
                ("b".into(), both),
                (" c".into(), st(Modifier::BOLD)),
            ]
        );
    }

    #[test]
    fn escaped_pipes_stay_inside_table_cells() {
        assert_eq!(table_cells(r"| a \| b | c |"), [r"a \| b", "c"]);
        assert_eq!(table_cells(r"| [[N\|alias]] |"), [r"[[N\|alias]]"]);
        assert_eq!(
            plain(&Line::from(inline_spans(r"[[N\|alias]]", Style::default()))),
            "alias"
        );
        assert_eq!(
            plain(&Line::from(inline_spans(r"a \| b", Style::default()))),
            "a | b"
        );
    }

    #[test]
    fn embed_links_show_an_embed_marker() {
        let link = Style::default().fg(Color::LightBlue);
        assert_eq!(
            styled("see ![[Note|shown]] here"),
            [
                ("see ".into(), Style::default()),
                ("⧉ shown".into(), link),
                (" here".into(), Style::default()),
            ]
        );
    }

    #[test]
    fn markup_nests() {
        assert_eq!(
            styled("~~a **b** c~~"),
            [
                ("a ".into(), st(Modifier::CROSSED_OUT)),
                ("b".into(), st(Modifier::CROSSED_OUT | Modifier::BOLD)),
                (" c".into(), st(Modifier::CROSSED_OUT)),
            ]
        );
    }

    #[test]
    fn inline_keeps_strikethrough_of_done_task() {
        let l = render_line("- [x] ping [[Bob]]");
        let link = l.spans.last().unwrap();
        assert_eq!(link.content, "Bob");
        assert!(link.style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn quotes_and_callouts() {
        assert_eq!(plain(&render_line("> just a quote")), "┃ just a quote");
        assert_eq!(plain(&render_line("> [!todo]+ Today")), "┃ ☑ Today");
        assert_eq!(plain(&render_line("> [!quote] Words")), "┃ ❝ Words");
        assert_eq!(plain(&render_line("> [!tip]")), "┃ ✦ TIP");
    }

    #[test]
    fn frontmatter_render() {
        assert_eq!(
            plain(&render_frontmatter_line("status: active", None)),
            "status active"
        );
        assert_eq!(
            plain(&render_frontmatter_line("  - one", Some("list"))),
            "  one"
        );
    }

    #[test]
    fn render_hides_markers() {
        assert_eq!(plain(&render_line("# Hello")), "█ HELLO");
        assert_eq!(plain(&render_line("## Sub")), "▌ Sub");
        assert_eq!(plain(&render_line("#### Deep")), "Deep");
        assert_eq!(plain(&render_line("- a")), "• a");
        assert_eq!(plain(&render_line("  - b")), "│ ◦ b");
        assert_eq!(plain(&render_line("    - c")), "│ │ ▪ c");
        assert_eq!(plain(&render_line("\t- d")), "│ ◦ d");
        assert_eq!(plain(&render_line("text")), "text");
    }

    #[test]
    fn source_render_is_verbatim() {
        for src in [
            "# Hello",
            "  - item",
            "plain",
            "### ünïcode",
            "- [x] t",
            "- [ ]",
            "> q",
            "---",
        ] {
            assert_eq!(plain(&render_source_line(src)), src);
        }
        assert_eq!(plain(&render_source_line("\t- [.] x")), "    - [.] x");
    }
}
