//! What's drawn over the document: the view's own prompts (search,
//! replace, the emoji picker), and the mdedit program's dialogs (save
//! prompts, the file browser). Everything draws into a buffer area, so an
//! editor can sit in any part of a host's screen.

use super::*;

/// A centered box `width` × `height` inside `area` (clamped to it), cleared.
fn popup_area(buf: &mut Buffer, area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2)).max(1);
    let h = height.min(area.height.saturating_sub(2)).max(1);
    let rect = Rect::new(
        area.x + area.width.saturating_sub(w) / 2,
        area.y + area.height.saturating_sub(h) / 2,
        w,
        h,
    );
    Clear.render(rect, buf);
    rect
}

/// Draws the view's prompt, if any: search and replace in `row` (the
/// status row), the emoji picker centered in `area`. Returns where the
/// cursor goes (the end of the input field).
pub(super) fn draw_view_popup(
    buf: &mut Buffer,
    area: Rect,
    row: Rect,
    view: &EditorView,
) -> Option<Position> {
    let prompt = Style::default().bg(Color::Blue).fg(Color::White);
    match &view.mode {
        ViewMode::Edit => None,
        ViewMode::EmojiPicker(p) => Some(draw_emoji_picker(buf, area, p)),
        ViewMode::Replace(r) => {
            let (find, with) = (" Find: ", " │ Replace: ");
            let text = format!(
                "{find}{}{with}{}   {}   Enter replace · ^A all · Tab field · ↑↓ skip · Esc close",
                r.find, r.with, view.status
            );
            Paragraph::new(text).style(prompt).render(row, buf);
            let mut x = find.width() + r.find.width();
            if r.editing_with {
                x += with.width() + r.with.width();
            }
            let x = row.x + x as u16;
            Some(Position::new(x.min(row.right().saturating_sub(1)), row.y))
        }
        ViewMode::Search(s) => {
            let label = " Search: ";
            let text = format!(
                "{label}{}   {}   ↑↓ previous / next · Enter done · Esc back",
                s.query, view.status
            );
            Paragraph::new(text).style(prompt).render(row, buf);
            let x = row.x + (label.width() + s.query.width()) as u16;
            Some(Position::new(x.min(row.right().saturating_sub(1)), row.y))
        }
    }
}

/// Draws the mdedit program's dialog, if any, centered on `screen`.
/// Returns where the cursor goes when a text field has focus.
pub(super) fn draw_dialog(buf: &mut Buffer, screen: Rect, app: &App) -> Option<Position> {
    let block = |title: &str| {
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(Style::default().fg(Color::Cyan))
    };
    match &app.mode {
        Mode::Edit => None,
        Mode::SavePrompt { then } => {
            let consequence = match then {
                After::Quit => "No: exit without saving",
                After::Open => "No: discard the changes when another file is opened",
                After::FollowLink => "No: discard the changes and follow the link",
                After::Edit => "",
            };
            let following = match (then, &app.pending_link) {
                (After::FollowLink, Some((target, _))) => format!(
                    "Following link to {}. ",
                    target.file_name().unwrap_or_default().to_string_lossy()
                ),
                _ => String::new(),
            };
            let text = vec![
                Line::from(format!("{following}Save changes to {}?", app.view.title())),
                Line::from(Span::styled(
                    "[Y]es   [N]o   [C]ancel",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    consequence,
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let area = popup_area(buf, screen, 72, 5);
            Paragraph::new(text)
                .block(block("Unsaved changes"))
                .render(area, buf);
            None
        }
        Mode::ConfirmOverwrite { path, .. } => {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let area = popup_area(buf, screen, 50, 3);
            Paragraph::new(format!("Overwrite {name}? (y/n)"))
                .block(block("File exists"))
                .render(area, buf);
            None
        }
        Mode::Browser(b) => Some(draw_browser(buf, screen, b)),
    }
}

/// The emoji picker (T-14): search field, a grid of results with the
/// highlighted emoji reversed, and a footer with the count and keys.
fn draw_emoji_picker(buf: &mut Buffer, screen: Rect, p: &Picker) -> Position {
    let width = (emoji::COLUMNS * 3 + 2) as u16;
    let area = popup_area(
        buf,
        screen,
        width,
        screen.height.saturating_sub(4).clamp(6, 18),
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Emoji ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    block.render(area, buf);

    let dim = Style::default().fg(Color::DarkGray);
    let label = "Search: ";
    let mut lines = vec![
        Line::from(vec![
            Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(p.query.clone()),
        ]),
        Line::from(Span::styled("─".repeat(inner.width as usize), dim)),
    ];
    let grid_rows = (inner.height as usize).saturating_sub(3).max(1);
    let selected_row = p.selected / emoji::COLUMNS;
    let first_row = selected_row.saturating_sub(grid_rows - 1);
    for chunk in p
        .results
        .chunks(emoji::COLUMNS)
        .enumerate()
        .skip(first_row)
        .take(grid_rows)
    {
        let (row, emojis) = chunk;
        let spans = emojis.iter().enumerate().map(|(col, e)| {
            let style = if row * emoji::COLUMNS + col == p.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Span::styled(format!("{} ", e.as_str()), style)
        });
        lines.push(Line::from(spans.collect::<Vec<_>>()));
    }
    while lines.len() < inner.height.saturating_sub(1) as usize {
        lines.push(Line::default());
    }
    let footer = if p.results.is_empty() {
        "No matches · Esc/^E close".to_string()
    } else {
        format!(
            "{} · ←↑↓→ move · Enter insert · Esc/^E close",
            p.results.len()
        )
    };
    lines.push(Line::from(Span::styled(footer, dim)));
    Paragraph::new(lines).render(inner, buf);

    let x = inner.x + (label.width() + p.query.width()) as u16;
    Position::new(x.min(inner.right().saturating_sub(1)), inner.y)
}

fn draw_browser(buf: &mut Buffer, screen: Rect, b: &Browser) -> Position {
    let (title, label) = match b.purpose {
        Purpose::SaveAs { .. } => ("Save As", "Name: "),
        Purpose::Open => ("Open", "Filter: "),
    };
    let area = popup_area(buf, screen, 70, screen.height.saturating_sub(4).max(8));
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    block.render(area, buf);

    let dim = Style::default().fg(Color::DarkGray);
    let mut lines = vec![
        Line::from(Span::styled(b.dir.display().to_string(), dim)),
        Line::from(vec![
            Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(b.input.clone()),
        ]),
        Line::from(Span::styled("─".repeat(inner.width as usize), dim)),
    ];
    let footer = 1 + usize::from(b.error.is_some());
    let list_rows = (inner.height as usize).saturating_sub(lines.len() + footer);
    let first = b
        .selected
        .map_or(0, |s| (s + 1).saturating_sub(list_rows.max(1)));
    for (i, e) in b.entries.iter().enumerate().skip(first).take(list_rows) {
        let name = if e.is_dir {
            format!("{}/", e.name)
        } else {
            e.name.clone()
        };
        let style = match (b.selected == Some(i), e.is_dir) {
            (true, _) => Style::default().add_modifier(Modifier::REVERSED),
            (false, true) => Style::default().fg(Color::Cyan),
            (false, false) => Style::default(),
        };
        lines.push(Line::from(Span::styled(format!(" {name}"), style)));
    }
    while lines.len() < inner.height as usize - footer.min(inner.height as usize) {
        lines.push(Line::default());
    }
    if let Some(err) = &b.error {
        lines.push(Line::from(Span::styled(
            err.clone(),
            Style::default().fg(Color::Red),
        )));
    }
    lines.push(Line::from(Span::styled(
        "Enter select · ↑↓ move · Ctrl+H all files · Esc cancel",
        dim,
    )));
    Paragraph::new(lines).render(inner, buf);

    let x = inner.x + (label.width() + b.input.width()) as u16;
    Position::new(x.min(inner.right().saturating_sub(1)), inner.y + 1)
}
