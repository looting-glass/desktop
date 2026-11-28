//! Making the game copy the hovered item: sending it Ctrl+C.

mod hyprctl;
mod remote_desktop;
mod virtual_keyboard;

use super::facts::Facts;
use super::select::Role;
use anyhow::Result;
use std::time::Duration;

/// Linux input event codes (`linux/input-event-codes.h`), which the portal and the virtual
/// keyboard both take.
const KEY_LEFTCTRL: u32 = 29;
const KEY_C: u32 = 46;

/// Ctrl+C as key events in order: (key, pressed).
const COPY_CHORD: [(u32, bool); 4] = [(KEY_LEFTCTRL, true), (KEY_C, true), (KEY_C, false), (KEY_LEFTCTRL, false)];

/// Games often sample key state once per frame, so space key events out by more than a frame.
const KEY_GAP: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySender {
    /// Hyprland's `sendshortcut` dispatcher, aimed at the game window itself.
    Hyprctl,
    /// The RemoteDesktop portal's virtual keyboard (KDE, GNOME), typing into the focused window.
    RemoteDesktop,
    /// A `zwp_virtual_keyboard_v1` keyboard (Sway, Umbriel, other wlroots-style compositors),
    /// typing into the focused window.
    VirtualKeyboard,
}

impl Role for KeySender {
    // The portal goes before the protocol: it's what KDE and GNOME offer apps for sending keys
    // (the user approves it once), and KDE's tested setup relies on it.
    const PRIORITY: &'static [Self] = &[Self::Hyprctl, Self::RemoteDesktop, Self::VirtualKeyboard];

    fn available(self, facts: &Facts) -> bool {
        match self {
            Self::Hyprctl => facts.hyprland_ipc,
            Self::RemoteDesktop => facts.remote_desktop_portal.is_some(),
            Self::VirtualKeyboard => facts.has_global("zwp_virtual_keyboard_manager_v1"),
        }
    }
}

impl KeySender {
    /// How the settings window and the docs name it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hyprctl => "hyprctl",
            Self::RemoteDesktop => "remote desktop portal",
            Self::VirtualKeyboard => "virtual keyboard",
        }
    }

    /// Get ready before the first price check, where that takes a while.
    pub fn warm_up(self) {
        match self {
            Self::Hyprctl | Self::VirtualKeyboard => {}
            Self::RemoteDesktop => {
                tokio::spawn(remote_desktop::warm_up());
            }
        }
    }

    pub async fn send_copy(self) -> Result<()> {
        match self {
            Self::Hyprctl => hyprctl::send_copy().await,
            Self::RemoteDesktop => remote_desktop::send_copy().await,
            Self::VirtualKeyboard => tokio::task::spawn_blocking(virtual_keyboard::send_copy).await?,
        }
    }
}
