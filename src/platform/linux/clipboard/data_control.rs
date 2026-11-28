//! The clipboard through the ext/wlr data-control protocols (via wl-clipboard-rs), which work
//! without having focus.

use std::io::Read;

pub fn clear() {
    use wl_clipboard_rs::copy::{ClipboardType, Seat, clear};
    if let Err(e) = clear(ClipboardType::Regular, Seat::All) {
        log::warn!("clipboard clear: {e}");
    }
}

pub fn read() -> Option<String> {
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};
    let (mut pipe, _) = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text)
        .map_err(|e| log::debug!("clipboard read: {e}"))
        .ok()?;
    let mut s = String::new();
    pipe.read_to_string(&mut s).ok()?;
    Some(s)
}
