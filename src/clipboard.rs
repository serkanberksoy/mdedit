//! The system clipboard, for Ctrl+V. A terminal program only gets keys, so
//! mdedit reads the clipboard itself, through the desktop's clipboard
//! tools. A host (or a test) can plug in its own [`Clipboard`].

/// Where Ctrl+V gets its text.
pub trait Clipboard {
    /// The clipboard's text; `None` if it's empty or can't be read.
    fn read(&self) -> Option<String>;
}

/// The desktop clipboard, read with the first tool that works:
/// `wl-paste` (Wayland), `xclip` or `xsel` (X11), `pbpaste` (macOS).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClipboard;

/// The tools, with their arguments, in the order they're tried.
const TOOLS: [&[&str]; 4] = [
    &["wl-paste", "--no-newline"],
    &["xclip", "-selection", "clipboard", "-o"],
    &["xsel", "--clipboard", "--output"],
    &["pbpaste"],
];

impl Clipboard for SystemClipboard {
    fn read(&self) -> Option<String> {
        TOOLS.iter().find_map(|tool| {
            let out = std::process::Command::new(tool[0])
                .args(&tool[1..])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()?;
            let text = String::from_utf8(out.stdout).ok()?;
            (out.status.success() && !text.is_empty()).then_some(text)
        })
    }
}
