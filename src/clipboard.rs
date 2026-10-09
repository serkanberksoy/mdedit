//! The system clipboard, for Ctrl+V. A terminal program only gets keys, so
//! mdedit reads the clipboard itself, through the desktop's clipboard
//! tools. A host (or a test) can plug in its own [`Clipboard`].

/// Where Ctrl+V gets its text.
pub trait Clipboard {
    /// The clipboard's text; `None` if it's empty or can't be read.
    fn read(&self) -> Option<String>;
}

/// The desktop clipboard, read with the first tool that works:
/// `wl-paste` (Wayland), `xclip` or `xsel` (X11), `pbpaste` (macOS),
/// PowerShell (Windows) ([`crate::platform::paste_tools`]). Windows'
/// `\r\n` line ends come in as `\n`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn read(&self) -> Option<String> {
        let tools = crate::platform::paste_tools(crate::platform::Os::this());
        tools.iter().find_map(|tool| {
            let out = std::process::Command::new(&tool[0])
                .args(&tool[1..])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()?;
            let text = String::from_utf8(out.stdout).ok()?.replace("\r\n", "\n");
            (out.status.success() && !text.is_empty()).then_some(text)
        })
    }
}
