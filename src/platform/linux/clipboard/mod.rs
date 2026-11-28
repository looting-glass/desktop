//! Reading what the game copied. The overlay doesn't have keyboard focus while the game copies,
//! and Wayland only lets a focused window read the clipboard, so this needs a protocol for
//! clipboard managers.

mod data_control;

use super::facts::Facts;
use super::select::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clipboard {
    /// The ext or wlr data-control protocol (KDE, wlroots compositors, Hyprland).
    DataControl,
}

impl Role for Clipboard {
    const PRIORITY: &'static [Self] = &[Self::DataControl];

    fn available(self, facts: &Facts) -> bool {
        match self {
            Self::DataControl => {
                facts.has_global("ext_data_control_manager_v1") || facts.has_global("zwlr_data_control_manager_v1")
            }
        }
    }
}

impl Clipboard {
    /// Empty the clipboard. Blocking.
    pub fn clear(self) {
        match self {
            Self::DataControl => data_control::clear(),
        }
    }

    /// The clipboard's text, if it holds any. Blocking.
    pub fn read(self) -> Option<String> {
        match self {
            Self::DataControl => data_control::read(),
        }
    }
}
