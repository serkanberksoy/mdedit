//! Image embeds (E-04, E-05): a line that is just `![[photo.png]]` or
//! `![alt](photo.png)` shows the picture below a title row, sized in
//! terminal cells. The pixels are drawn by `ratatui-image` (kitty, sixel,
//! iTerm2 or Unicode half blocks, whatever the terminal supports); this
//! module finds image links, loads the files and works out the size.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use image::DynamicImage;
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::widgets::Widget;
use ratatui_image::Resize;
use ratatui_image::picker::Picker;
use ratatui_image::sliced::{SignedPosition, SlicedImage, SlicedProtocol};

use crate::wrap::ImageSlot;

use crate::links::percent_decode;

/// File extensions shown as images.
const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"];

/// The most rows an image takes, so one picture never fills the screen.
pub const MAX_ROWS: u16 = 20;

/// An image embed line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageLink {
    /// The file (relative to the note) or web address, as written.
    pub target: String,
    /// The alt text (`![alt](…)`); empty for `![[…]]`.
    pub alt: String,
    /// Width and height in pixels from `|200` or `|200x100` (E-05).
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl ImageLink {
    /// Whether the image is on the web (not downloaded; shown as a title).
    pub fn is_remote(&self) -> bool {
        self.target.starts_with("http://") || self.target.starts_with("https://")
    }

    /// The file name to show in the title.
    pub fn name(&self) -> &str {
        self.target.rsplit('/').next().unwrap_or(&self.target)
    }
}

/// The image a line embeds, if the line is just an image embed:
/// `![[photo.png]]`, `![[photo.png|200]]`, `![[photo.png|200x100]]`,
/// `![alt](photo.png)` or `![alt|200](photo.png)`.
pub fn image_link(line: &str) -> Option<ImageLink> {
    let line = line.trim();
    if let Some(inner) = line.strip_prefix("![[").and_then(|r| r.strip_suffix("]]")) {
        if inner.contains("]]") || inner.contains("[[") {
            return None;
        }
        let (target, size) = match inner.split_once('|') {
            Some((target, size)) => (target, parse_size(size)?),
            None => (inner, (None, None)),
        };
        return has_image_extension(target).then(|| ImageLink {
            target: target.to_string(),
            alt: String::new(),
            width: size.0,
            height: size.1,
        });
    }
    let rest = line.strip_prefix("![")?;
    let (label, rest) = rest.split_once("](")?;
    let url = rest.strip_suffix(')')?;
    let url = match url.strip_prefix('<').and_then(|u| u.strip_suffix('>')) {
        Some(bracketed) => bracketed,
        None if url.contains([')', ' ', '\t']) => return None,
        None => url,
    };
    // `![alt|200](…)`: the size goes after the alt text.
    let (alt, size) = match label.rsplit_once('|') {
        Some((alt, size)) if parse_size(size).is_some() => (alt, parse_size(size)?),
        _ => (label, (None, None)),
    };
    let remote = url.starts_with("http://") || url.starts_with("https://");
    (remote || has_image_extension(url)).then(|| ImageLink {
        target: url.to_string(),
        alt: alt.to_string(),
        width: size.0,
        height: size.1,
    })
}

/// `200` or `200x100` (pixels).
fn parse_size(size: &str) -> Option<(Option<u32>, Option<u32>)> {
    match size.trim().split_once('x') {
        Some((w, h)) => Some((Some(w.parse().ok()?), Some(h.parse().ok()?))),
        None => Some((Some(size.trim().parse().ok()?), None)),
    }
}

/// Whether `target` names an image file (by its extension).
fn has_image_extension(target: &str) -> bool {
    let path = target.split(['?', '#']).next().unwrap_or(target);
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// The size in terminal cells (columns, rows) of an image of `pixels`,
/// shown at `wanted` pixels if given (E-05), with cells of `cell` pixels,
/// at most `max_cols` wide and [`MAX_ROWS`] tall. The aspect ratio is kept.
pub fn cell_size(
    pixels: (u32, u32),
    wanted: (Option<u32>, Option<u32>),
    cell: (u16, u16),
    max_cols: u16,
) -> (u16, u16) {
    let (iw, ih) = (f64::from(pixels.0.max(1)), f64::from(pixels.1.max(1)));
    let (mut pw, mut ph) = match wanted {
        (Some(w), Some(h)) => (f64::from(w), f64::from(h)),
        (Some(w), None) => (f64::from(w), f64::from(w) * ih / iw),
        (None, Some(h)) => (f64::from(h) * iw / ih, f64::from(h)),
        (None, None) => (iw, ih),
    };
    let (cw, ch) = (f64::from(cell.0.max(1)), f64::from(cell.1.max(1)));
    let cells = |pixels: f64, cell: f64| (pixels / cell).ceil().max(1.0);
    // Too wide or too tall: scale down, keeping the shape.
    let max_cols = f64::from(max_cols.max(1));
    if cells(pw, cw) > max_cols {
        ph *= max_cols * cw / pw;
        pw = max_cols * cw;
    }
    let max_rows = f64::from(MAX_ROWS);
    if cells(ph, ch) > max_rows {
        pw *= max_rows * ch / ph;
        ph = max_rows * ch;
    }
    // At most MAX_ROWS × max_cols, so these fit in a u16.
    (cells(pw, cw) as u16, cells(ph, ch) as u16)
}

/// A decoded image file and when it was read.
type Entry = (Option<SystemTime>, u64, Option<Arc<DynamicImage>>);

/// The decoded image at `path`, cached until the file changes. `None` if
/// the file is missing or isn't an image this build can decode.
pub fn load(path: &Path) -> Option<Arc<DynamicImage>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    let meta = std::fs::metadata(path).ok()?;
    let stamp = (meta.modified().ok(), meta.len());
    let cache = CACHE.get_or_init(Default::default);
    if let Some((m, s, image)) = cache.lock().ok()?.get(path)
        && (*m, *s) == stamp
    {
        return image.clone();
    }
    let image = image::open(path).ok().map(Arc::new);
    cache
        .lock()
        .ok()?
        .insert(path.to_path_buf(), (stamp.0, stamp.1, image.clone()));
    image
}

/// Images encoded for the terminal, by file and size in cells. Encoding
/// (resizing, sixel or kitty data) is slow, so it's done once per image
/// and size, and again only when the file changes.
#[derive(Default)]
pub struct ImageCache {
    entries: HashMap<(PathBuf, u16, u16), (Arc<DynamicImage>, SlicedProtocol)>,
}

impl ImageCache {
    /// Images kept; the cache starts over when it has more.
    const LIMIT: usize = 64;

    /// The encoded image for `slot`, if its file can be shown.
    fn get(&mut self, picker: &Picker, slot: &ImageSlot) -> Option<&SlicedProtocol> {
        let image = load(&slot.path)?;
        let key = (slot.path.clone(), slot.cols, slot.rows);
        let fresh = self
            .entries
            .get(&key)
            .is_some_and(|(cached, _)| Arc::ptr_eq(cached, &image));
        if !fresh {
            let size = Size::new(slot.cols, slot.rows);
            let resize = Resize::Fit(None);
            let protocol =
                SlicedProtocol::new_with_resize(picker, (*image).clone(), size, resize).ok()?;
            if self.entries.len() >= Self::LIMIT {
                self.entries.clear();
            }
            self.entries.insert(key.clone(), (image, protocol));
        }
        self.entries.get(&key).map(|(_, protocol)| protocol)
    }
}

/// Draws the images of the visible `slots` (screen row of the image's top,
/// relative to `area`, and the slot) over `area`; an image that runs past
/// the bottom is cut off. Returns the screen areas the images cover.
pub fn draw_images(
    buf: &mut Buffer,
    area: Rect,
    slots: &[(usize, ImageSlot)],
    picker: &Picker,
    cache: &mut ImageCache,
) -> Vec<Rect> {
    let mut covered = Vec::new();
    for (y, slot) in slots {
        let Some(protocol) = cache.get(picker, slot) else {
            continue;
        };
        let (Ok(x), Ok(y)) = (i16::try_from(slot.col), i16::try_from(*y)) else {
            continue;
        };
        SlicedImage::new(protocol, SignedPosition::from((x, y))).render(area, buf);
        let top = area.y + y as u16;
        let rows = slot.rows.min(area.bottom().saturating_sub(top));
        let cols = slot.cols.min(area.width.saturating_sub(slot.col));
        covered.push(Rect::new(area.x + slot.col, top, cols, rows));
    }
    covered
}

/// The image picker for the `images` setting: `None` when images are off.
/// `Auto` and the protocol choices ask the terminal (with a short timeout)
/// for its cell size in pixels and what it supports; a terminal that
/// doesn't answer gets half blocks.
pub fn picker_for(images: crate::config::Images) -> Option<Picker> {
    use crate::config::Images;
    use ratatui_image::picker::ProtocolType;
    use ratatui_image::picker::cap_parser::QueryStdioOptions;
    let forced = match images {
        Images::Off => return None,
        Images::Halfblocks => return Some(Picker::halfblocks()),
        Images::Auto => None,
        Images::Kitty => Some(ProtocolType::Kitty),
        Images::Sixel => Some(ProtocolType::Sixel),
        Images::Iterm2 => Some(ProtocolType::Iterm2),
    };
    let options = QueryStdioOptions {
        timeout: std::time::Duration::from_millis(500),
        ..QueryStdioOptions::default()
    };
    let mut picker =
        Picker::from_query_stdio_with_options(options).unwrap_or_else(|_| Picker::halfblocks());
    if let Some(protocol) = forced {
        picker.set_protocol_type(protocol);
    }
    Some(picker)
}

/// Decodes `target` for a link written in a note (`%20` → space).
pub fn decoded_target(link: &ImageLink) -> String {
    percent_decode(&link.target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(target: &str, alt: &str, width: Option<u32>, height: Option<u32>) -> ImageLink {
        ImageLink {
            target: target.into(),
            alt: alt.into(),
            width,
            height,
        }
    }

    #[test]
    fn wiki_image_embeds_with_and_without_a_size() {
        assert_eq!(
            image_link("![[diagram.png]]"),
            Some(link("diagram.png", "", None, None))
        );
        assert_eq!(
            image_link("  ![[img/My Photo.JPG|200]] "),
            Some(link("img/My Photo.JPG", "", Some(200), None))
        );
        assert_eq!(
            image_link("![[diagram.png|200x100]]"),
            Some(link("diagram.png", "", Some(200), Some(100)))
        );
    }

    #[test]
    fn markdown_image_embeds() {
        assert_eq!(
            image_link("![Alt text](https://example.com/image.png)"),
            Some(link(
                "https://example.com/image.png",
                "Alt text",
                None,
                None
            ))
        );
        assert_eq!(
            image_link("![Alt|200](pics/a%20b.png)"),
            Some(link("pics/a%20b.png", "Alt", Some(200), None))
        );
        assert_eq!(
            image_link("![](<pics/a b.png>)"),
            Some(link("pics/a b.png", "", None, None))
        );
    }

    #[test]
    fn not_image_embeds() {
        for line in [
            "![[note]]",
            "![[note.md]]",
            "see ![[diagram.png]] inline",
            "![[a.png]] ![[b.png]]",
            "[[diagram.png]]",
            "![doc](file.pdf)",
            "text",
        ] {
            assert_eq!(image_link(line), None, "{line}");
        }
    }

    #[test]
    fn remote_and_name() {
        let web = image_link("![x](https://example.com/p/image.png)").unwrap();
        assert!(web.is_remote());
        assert_eq!(web.name(), "image.png");
        assert!(!image_link("![[a/b.png]]").unwrap().is_remote());
    }

    #[test]
    fn natural_size_in_cells() {
        // 100×40 px with 10×20 px cells: 10 columns, 2 rows.
        assert_eq!(cell_size((100, 40), (None, None), (10, 20), 80), (10, 2));
        // Partial cells round up.
        assert_eq!(cell_size((101, 41), (None, None), (10, 20), 80), (11, 3));
    }

    #[test]
    fn wanted_size_keeps_the_aspect_ratio_unless_both_are_given() {
        assert_eq!(
            cell_size((100, 40), (Some(200), None), (10, 20), 80),
            (20, 4)
        );
        assert_eq!(
            cell_size((100, 40), (Some(200), Some(100)), (10, 20), 80),
            (20, 5)
        );
    }

    #[test]
    fn big_images_fit_the_width_and_the_row_limit() {
        // 2000×400 px would be 200 columns: scaled to 50, keeping the shape.
        assert_eq!(cell_size((2000, 400), (None, None), (10, 20), 50), (50, 5));
        // 400×4000 px would be 200 rows: scaled to MAX_ROWS.
        assert_eq!(
            cell_size((400, 4000), (None, None), (10, 20), 80),
            (4, MAX_ROWS)
        );
        assert_eq!(
            cell_size((1, 1), (None, None), (10, 20), 80),
            (1, 1),
            "at least a cell"
        );
    }
}
