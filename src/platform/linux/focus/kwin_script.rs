//! Give keyboard focus back to the game via a one-shot KWin script.
//!
//! The overlay takes keyboard focus (so Esc/Space work). When a layer-shell surface closes,
//! KWin does not reliably refocus the game (it may pick a window on another output), and
//! Wayland clients cannot activate other windows themselves.

use anyhow::{Context, Result};
use ashpd::zbus::Connection;

const SCRIPT_NAME: &str = "lootingglass-focus-game";
const SCRIPT: &str = r#"
const re = /^Path of Exile/;
const w = workspace.windowList().find(w => re.test(w.caption) || /pathofexile|steam_app_(238960|2694490)$/i.test(w.resourceClass));
if (w && workspace.activeWindow !== w) workspace.activeWindow = w;
"#;

pub async fn focus_game() {
    if let Err(e) = run().await {
        log::warn!("could not refocus the game via KWin: {e:#}");
    }
}

async fn run() -> Result<()> {
    let path = dirs::state_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("lootingglass/focus-game.js");
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, SCRIPT)?;
    let conn = Connection::session().await?;
    // A previous run may have left the script loaded; unload so loadScript returns a fresh id.
    let _ = conn
        .call_method(Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(SCRIPT_NAME,))
        .await;
    let reply = conn
        .call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "loadScript",
            &(path.to_string_lossy().as_ref(), SCRIPT_NAME),
        )
        .await
        .context("loadScript")?;
    let id: i32 = reply.body().deserialize()?;
    let script_path = format!("/Scripting/Script{id}");
    conn.call_method(Some("org.kde.KWin"), script_path.as_str(), Some("org.kde.kwin.Script"), "run", &())
        .await
        .context("run")?;
    let _ = conn
        .call_method(Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(SCRIPT_NAME,))
        .await;
    Ok(())
}
