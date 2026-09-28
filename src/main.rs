//! mdedit: terminal Markdown editor with Obsidian-style live preview.
//!
//! Usage: mdedit [FILE]

use std::io::{self, Write};
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::cursor::MoveTo;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::style::Print;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::crossterm::{execute, queue};

use mdedit::app::{Action, App};
use mdedit::cli;
use mdedit::config::{self, Config};
use mdedit::emoji::{self, Recent};
use mdedit::links;
use mdedit::terminal::Capabilities;
use mdedit::ui;
use mdedit::wrap::LineAttr;

fn main() -> io::Result<()> {
    // `args_os`: a non-UTF-8 argument must not panic (it's converted lossily).
    let args = std::env::args_os().skip(1).map(|a| {
        a.into_string()
            .unwrap_or_else(|a| a.to_string_lossy().into_owned())
    });
    let cli = match cli::parse(args) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("mdedit: {e}");
            std::process::exit(2);
        }
    };
    if cli.help {
        print!("{}", cli::USAGE);
        return Ok(());
    }
    if cli.version {
        println!("mdedit {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    // `mdedit note.md#Heading` opens at that heading (F-06).
    let (path, section) = match cli.file.as_deref() {
        Some(arg) => {
            let (file, heading) = links::split_section(arg);
            (Some(file), heading)
        }
        None => (None, None),
    };
    let text = match &path {
        Some(p) => match std::fs::read_to_string(p) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        },
        None => String::new(),
    };

    let mut app = App::new(&text, path);
    app.shared.caps = Capabilities::from_env();
    if let Some(recent) = emoji::default_recent_path() {
        app.shared.recent = Recent::load(recent);
    }
    if let Some(path) = config::default_path() {
        let (settings, warnings) = Config::load(&path);
        app.shared.config = settings;
        app.view.status = warnings.join("; ");
    }
    app.view.source_mode = cli.source;
    if let Some(size) = cli.heading_size {
        app.shared.config.heading_size = size;
    }
    if let Some(heading) = section {
        app.go_to_heading(&heading);
    }
    let mut terminal = ratatui::init();
    // Pasted text arrives as one event instead of keystrokes, so newlines
    // don't continue lists and tabs don't indent.
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    // Lets Ctrl+Shift+S (Save As) be told apart from Ctrl+S, in terminals
    // that support the kitty keyboard protocol. Ctrl+Alt+S works everywhere.
    let enhanced = supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok();
    // How images are drawn (E-04): asks the terminal, so after raw mode.
    app.shared.picker = mdedit::images::picker_for(app.shared.config.images);
    // A panic must not leave the terminal in the kitty keyboard or paste
    // mode (ratatui's own hook restores the rest of the terminal).
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        leave_modes(enhanced);
        hook(info);
    }));
    let result = run(&mut terminal, &mut app);
    leave_modes(enhanced);
    ratatui::restore();
    result
}

/// Turns off the terminal modes `main` turned on.
fn leave_modes(enhanced: bool) {
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    if enhanced {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    // Line sizes currently set on the terminal's rows (R-16).
    let mut shown: Vec<LineAttr> = Vec::new();
    let mut force = false;
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        if sync_line_sizes(&app.view.line_attrs, &mut shown, force)? {
            // Redraw so the text lands in rows with their new sizes.
            terminal.draw(|f| ui::draw(f, app))?;
        }
        force = false;

        // Handle every event that's already waiting before drawing again, so
        // a held key or a fast typist never waits for frames.
        let mut next = event::read()?;
        loop {
            match next {
                Event::Key(key)
                    if key.kind == KeyEventKind::Press && app.handle_key(key) == Action::Quit =>
                {
                    return Ok(());
                }
                Event::Paste(text) => app.handle_paste(&text),
                // The terminal clears the screen on resize: set sizes again.
                Event::Resize(..) => force = true,
                _ => {}
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
            next = event::read()?;
        }
    }
}

/// Sends DEC line-size codes (`ESC # 3/4/5/6`, R-16) for rows whose size
/// changed. Nothing is sent to a terminal that never shows double-size rows.
/// Returns whether anything was sent.
fn sync_line_sizes(
    wanted: &[LineAttr],
    shown: &mut Vec<LineAttr>,
    force: bool,
) -> io::Result<bool> {
    let any_big = |rows: &[LineAttr]| rows.iter().any(|a| *a != LineAttr::Normal);
    if !force && (wanted == shown.as_slice() || !(any_big(wanted) || any_big(shown))) {
        return Ok(false);
    }
    let mut out = io::stdout();
    for (y, attr) in wanted.iter().enumerate() {
        let code = match attr {
            LineAttr::Normal => "\x1b#5",
            LineAttr::DoubleTop => "\x1b#3",
            LineAttr::DoubleBottom => "\x1b#4",
            LineAttr::DoubleWide => "\x1b#6",
        };
        queue!(out, MoveTo(0, y as u16), Print(code))?;
    }
    out.flush()?;
    *shown = wanted.to_vec();
    Ok(true)
}
