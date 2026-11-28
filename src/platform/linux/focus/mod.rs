//! Giving keyboard focus back to the game (after the overlay closes, before copying). Wayland
//! clients can't activate another window themselves, so this needs the compositor's help.

mod hyprctl;
mod kwin_script;
mod sway_ipc;
mod umbriel_ipc;

use super::facts::Facts;
use super::select::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Hyprland's `focuswindow` dispatcher.
    Hyprctl,
    /// A one-shot KWin script.
    KWinScript,
    /// Sway's IPC, through `swaymsg`.
    SwayIpc,
    /// Umbriel's IPC, through the `umbriel` command.
    UmbrielIpc,
    /// No way to refocus the game: the user clicks it themselves.
    None,
}

impl Role for Focus {
    const PRIORITY: &'static [Self] = &[Self::Hyprctl, Self::KWinScript, Self::SwayIpc, Self::UmbrielIpc];

    fn available(self, facts: &Facts) -> bool {
        match self {
            Self::Hyprctl => facts.hyprland_ipc,
            Self::KWinScript => facts.kwin,
            Self::SwayIpc => facts.sway_ipc,
            Self::UmbrielIpc => facts.umbriel_ipc,
            Self::None => true,
        }
    }
}

impl Focus {
    /// How the settings window and the docs name it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hyprctl => "hyprctl",
            Self::KWinScript => "KWin script",
            Self::SwayIpc => "Sway IPC",
            Self::UmbrielIpc => "Umbriel IPC",
            Self::None => "none",
        }
    }

    pub async fn focus_game(self) {
        match self {
            Self::Hyprctl => hyprctl::focus_game().await,
            Self::KWinScript => kwin_script::focus_game().await,
            Self::SwayIpc => sway_ipc::focus_game().await,
            Self::UmbrielIpc => umbriel_ipc::focus_game().await,
            Self::None => {}
        }
    }
}
