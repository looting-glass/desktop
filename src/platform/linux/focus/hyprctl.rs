//! Focus the game through Hyprland's `focuswindow` dispatcher.

use crate::platform::linux::hyprctl::{GAME_WINDOW, dispatch};

pub async fn focus_game() {
    if let Err(e) = dispatch("focuswindow", GAME_WINDOW).await {
        log::warn!("could not refocus the game via Hyprland: {e:#}");
    }
}
