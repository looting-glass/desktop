//! The price overlay: a window kept above the game, even a fullscreen one. `cpp/main.cpp` sets
//! it up; this role records whether the desktop allows one, so Settings and the docs can say so.

use super::facts::Facts;
use super::select::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// A wlr-layer-shell surface, through LayerShellQt.
    LayerShell,
}

impl Role for Overlay {
    const PRIORITY: &'static [Self] = &[Self::LayerShell];

    fn available(self, facts: &Facts) -> bool {
        match self {
            Self::LayerShell => facts.has_global("zwlr_layer_shell_v1"),
        }
    }
}

impl Overlay {
    /// How the settings window and the docs name it.
    pub fn label(self) -> &'static str {
        match self {
            Self::LayerShell => "layer-shell",
        }
    }
}
