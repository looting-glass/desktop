//! Hyprland — EXPERIMENTAL, not yet tested on a real Hyprland session.
//!
//! Its portal has no RemoteDesktop, but Hyprland's own dispatchers can send a key combination
//! to a specific window and focus it, so we drive those through `hyprctl`.

use anyhow::{Context, Result, bail};

/// Hyprland window rule matching the PoE 1 / PoE 2 game window (not e.g. a "Trade - Path of
/// Exile" browser tab, thanks to the anchors).
const GAME_WINDOW: &str = "title:^(Path of Exile( 2)?)$";

/// Example `hyprland.conf` line shown to users (Hyprland ignores preferred triggers).
pub const BIND_HINT: &str = "bind = CTRL, D, global, dev.lootingglass.LootingGlass:price-check";

pub fn detected() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

async fn dispatch(dispatcher: &str, args: &str) -> Result<()> {
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

/// Send Ctrl+C straight to the game window.
pub async fn send_copy() -> Result<()> {
    dispatch("sendshortcut", &format!("CTRL, C, {GAME_WINDOW}"))
        .await
        .context("Could not send Ctrl+C to the game — is Path of Exile running?")
}

pub async fn focus_game() {
    if let Err(e) = dispatch("focuswindow", GAME_WINDOW).await {
        log::warn!("could not refocus the game via Hyprland: {e:#}");
    }
}
