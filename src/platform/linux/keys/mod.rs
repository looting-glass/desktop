//! Making the game copy the hovered item: sending it Ctrl+C.

mod hyprctl;
mod remote_desktop;

use super::facts::Facts;
use super::select::Role;
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySender {
    /// Hyprland's `sendshortcut` dispatcher, aimed at the game window itself.
    Hyprctl,
    /// The RemoteDesktop portal's virtual keyboard (KDE, GNOME), typing into the focused window.
    RemoteDesktop,
}

impl Role for KeySender {
    const PRIORITY: &'static [Self] = &[Self::Hyprctl, Self::RemoteDesktop];

    fn available(self, facts: &Facts) -> bool {
        match self {
            Self::Hyprctl => facts.hyprland_ipc,
            Self::RemoteDesktop => facts.remote_desktop_portal.is_some(),
        }
    }
}

impl KeySender {
    /// Get ready before the first price check, where that takes a while.
    pub fn warm_up(self) {
        match self {
            Self::Hyprctl => {}
            Self::RemoteDesktop => {
                tokio::spawn(remote_desktop::warm_up());
            }
        }
    }

    pub async fn send_copy(self) -> Result<()> {
        match self {
            Self::Hyprctl => hyprctl::send_copy().await,
            Self::RemoteDesktop => remote_desktop::send_copy().await,
        }
    }
}
