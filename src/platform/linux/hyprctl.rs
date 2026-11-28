//! Hyprland's `hyprctl` dispatchers, which the `Hyprctl` key sender and focus providers use.
//! EXPERIMENTAL, not yet tested on a real Hyprland session.

use anyhow::{Context, Result, bail};

/// Hyprland window rule matching the PoE 1 / PoE 2 game window (not e.g. a "Trade - Path of
/// Exile" browser tab, thanks to the anchors).
pub const GAME_WINDOW: &str = "title:^(Path of Exile( 2)?)$";

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
