//! Hyprland's `hyprctl` dispatchers, which the `Hyprctl` key sender and focus providers use.
//! EXPERIMENTAL, not yet tested on a real Hyprland session.

use super::{GAME_TITLE, cli};
use anyhow::{Result, bail};

/// Hyprland window rule matching the game window.
pub fn game_window() -> String {
    format!("title:{GAME_TITLE}")
}

pub async fn dispatch(dispatcher: &str, args: &str) -> Result<()> {
    // hyprctl exits successfully even when the dispatcher fails, and says so instead of "ok".
    let reply = cli::run("hyprctl", &["dispatch", dispatcher, args]).await?;
    if reply.trim() != "ok" {
        bail!("hyprctl dispatch {dispatcher} failed: {}", reply.trim());
    }
    Ok(())
}
