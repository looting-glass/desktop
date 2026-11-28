//! Giving keyboard focus back to the game (after the overlay closes, before copying). Wayland
//! clients can't activate another window themselves, so this needs the compositor's help.

mod hyprctl;
mod kwin_script;

use super::facts::Facts;
use super::select::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Hyprland's `focuswindow` dispatcher.
    Hyprctl,
    /// A one-shot KWin script.
    KWinScript,
    /// No way to refocus the game: the user clicks it themselves.
    None,
}

impl Role for Focus {
    const PRIORITY: &'static [Self] = &[Self::Hyprctl, Self::KWinScript];

    fn available(self, facts: &Facts) -> bool {
        match self {
            Self::Hyprctl => facts.hyprland_ipc,
            Self::KWinScript => facts.kwin,
            Self::None => true,
        }
    }
}

impl Focus {
    pub async fn focus_game(self) {
        match self {
            Self::Hyprctl => hyprctl::focus_game().await,
            Self::KWinScript => kwin_script::focus_game().await,
            Self::None => {}
        }
    }
}
