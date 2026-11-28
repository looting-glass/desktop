//! Ctrl+C through Hyprland's `sendshortcut` dispatcher. Hyprland's portal has no RemoteDesktop,
//! but its dispatcher can send a key combination straight to the game window.

use crate::platform::linux::hyprctl::{dispatch, game_window};
use anyhow::{Context, Result};

pub async fn send_copy() -> Result<()> {
    dispatch("sendshortcut", &format!("CTRL, C, {}", game_window()))
        .await
        .context("Could not send Ctrl+C to the game — is Path of Exile running?")
}
