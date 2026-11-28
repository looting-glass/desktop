//! Focus the game through Hyprland's `focuswindow` dispatcher.

use crate::platform::linux::hyprctl::{dispatch, game_window};

pub async fn focus_game() {
    if let Err(e) = dispatch("focuswindow", &game_window()).await {
        log::warn!("could not refocus the game via Hyprland: {e:#}");
    }
}
