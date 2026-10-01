//! What all open editors share: the settings, what the terminal can do,
//! how images are drawn, the recently used emoji and how links are found.
//! A host with several editors (tabs, panes) keeps one [`Shared`] and one
//! [`crate::view::EditorView`] per document.

use ratatui_image::picker::Picker as ImagePicker;

use crate::clipboard::{Clipboard, SystemClipboard};
use crate::config::Config;
use crate::emoji::Recent;
use crate::images::ImageCache;
use crate::markdown::Options;
use crate::processor::CodeBlockProcessor;
use crate::resolver::{RelativeResolver, Resolver};
use crate::terminal::Capabilities;

pub struct Shared {
    /// User settings (mdedit loads them from `config.toml`; tests and hosts
    /// start from the defaults).
    pub config: Config,
    /// What the terminal supports (detected at startup; tests use defaults).
    pub caps: Capabilities,
    /// How images are drawn (E-04); `None` when images are off. The default
    /// is half blocks; a host asks the terminal once at startup with
    /// [`crate::images::picker_for`].
    pub picker: Option<ImagePicker>,
    /// Images already encoded for the terminal.
    pub image_cache: ImageCache,
    /// Recently inserted emoji (EP-06).
    pub recent: Recent,
    /// How links, embeds and images find their files.
    pub resolver: Box<dyn Resolver>,
    /// Renders the host's own code blocks (e.g. plugin queries); `None`
    /// shows every fenced block as code.
    pub processor: Option<Box<dyn CodeBlockProcessor>>,
    /// Where Ctrl+V gets its text (the desktop clipboard by default).
    pub clipboard: Box<dyn Clipboard>,
    /// A host's colors for the editor (its theme); the default changes
    /// nothing.
    pub palette: crate::palette::Palette,
}

impl Shared {
    /// Default settings, half-block images and relative links.
    pub fn new() -> Self {
        Shared {
            config: Config::default(),
            caps: Capabilities::default(),
            picker: Some(ImagePicker::halfblocks()),
            image_cache: ImageCache::default(),
            recent: Recent::default(),
            resolver: Box::new(RelativeResolver::new()),
            processor: None,
            clipboard: Box::new(SystemClipboard),
            palette: crate::palette::Palette::default(),
        }
    }

    /// The rendering options for the settings and the terminal, with
    /// `source_mode` from the view.
    pub fn render_options(&self, source_mode: bool) -> Options {
        let mut options = self.config.options(&self.config.apply(self.caps));
        options.source_mode = source_mode;
        options
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
