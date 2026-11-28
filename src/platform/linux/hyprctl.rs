//! Hyprland's `hyprctl` dispatchers, which the `Hyprctl` key sender and focus providers use.
//! EXPERIMENTAL, not yet tested on a real Hyprland session.

use super::GAME_TITLE;
use anyhow::{Context, Result, bail};

/// Hyprland window rule matching the game window.
pub fn game_window() -> String {
    format!("title:{GAME_TITLE}")
}

pub async fn dispatch(dispatcher: &str, args: &str) -> Result<()> {
    let out = tokio::process::Command::new("hyprctl")
        .args(["dispatch", dispatcher, args])
        .output()
        .await
        .context("running hyprctl")?;
    let reply = String::from_utf8_lossy(&out.stdout);
    if reply.trim() != "ok" {
        bail!("hyprctl dispatch {dispatcher} failed: {}", reply.trim());
    }
    Ok(())
}
