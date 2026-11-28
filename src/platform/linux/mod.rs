//! Linux (Wayland). Each job the app needs from the desktop is a role with interchangeable
//! providers (`shortcuts`, `keys`, `focus`, `clipboard`). Which provider a session gets is
//! decided once, from what the desktop offers (`facts`), by the pure `select::select`; the
//! functions below delegate to that selection.

mod clipboard;
mod facts;
mod focus;
mod hyprctl;
mod keys;
mod notify;
mod select;
mod shortcuts;
mod tray;

use super::{ShortcutCheck, ShortcutList};
use crate::Event;
use anyhow::Result;
use facts::Facts;
use select::Selection;
use std::time::Duration;
use tokio::sync::OnceCell;

pub use notify::show as notify;
pub use tray::{set_game as set_tray_game, set_update as set_tray_update, spawn as spawn_tray};

/// Our app id, as the portals, kglobalaccel and the notification server know us.
const APP_ID: &str = "dev.lootingglass.LootingGlass";

/// The providers for this session, chosen on first use.
async fn desktop() -> &'static Selection {
    static SELECTION: OnceCell<Selection> = OnceCell::const_new();
    SELECTION
        .get_or_init(|| async {
            let facts = Facts::probe().await;
            log::debug!("{facts:?}");
            let selection = select::select(&facts);
            log::info!("desktop {:?}: {selection:?}", facts.desktop);
            selection
        })
        .await
}

pub async fn init() {
    register_app().await;
    if let Ok(keys) = desktop().await.keys {
        keys.warm_up();
    }
}

/// Tell the portals which .desktop file we are (needed for unsandboxed apps).
async fn register_app() {
    match APP_ID.try_into() {
        Ok(id) => {
            if let Err(e) = ashpd::register_host_app(id).await {
                log::warn!("portal host registration failed (old xdg-desktop-portal?): {e}");
            }
        }
        Err(e) => log::warn!("bad app id: {e}"),
    }
}

/// Give keyboard focus back to the game (after the overlay closes, before copying).
pub async fn focus_game() {
    desktop().await.focus.focus_game().await
}

/// Make the game copy the hovered item and return the clipboard text.
pub async fn copy_item() -> Result<String> {
    let desktop = desktop().await;
    let clipboard = desktop.clipboard?;
    let keys = desktop.keys?;
    // Clear first so a missed keystroke can't silently return the previous item.
    tokio::task::spawn_blocking(move || clipboard.clear()).await?;
    // The overlay may have held keyboard focus; make sure Ctrl+C reaches the game.
    desktop.focus.focus_game().await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    keys.send_copy().await?;
    // Wine/XWayland can take a while to publish the selection; poll for up to 1.5s.
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(now) = tokio::task::spawn_blocking(move || clipboard.read()).await?
            && !now.trim().is_empty()
        {
            return Ok(now);
        }
    }
    anyhow::bail!("The game didn't copy anything — is the cursor over an item and the game focused?")
}

pub async fn run_shortcuts(tx: async_channel::Sender<Event>) -> Result<()> {
    desktop().await.shortcuts?.run(tx).await
}

/// Shortcuts are bound in the desktop's settings (portal), nothing to reload.
pub fn reload_shortcuts() {}

/// Our global shortcuts for the settings window.
pub async fn shortcut_list() -> ShortcutList {
    match desktop().await.shortcuts {
        Ok(shortcuts) => shortcuts.list().await,
        Err(missing) => ShortcutList { mode: "portal", rows: shortcuts::rows(|_| String::new()), error: missing.to_string() },
    }
}

/// Bind `key` (a Qt key combination, 0 = none) to shortcut `id` (KDE only).
pub async fn set_shortcut(id: &str, key: i32) -> Result<()> {
    desktop().await.shortcuts?.set(id, key).await
}

/// How `key` reads ("Ctrl+D") and who else uses it, before it's saved (KDE only).
pub async fn check_shortcut(id: &str, key: i32) -> ShortcutCheck {
    match desktop().await.shortcuts {
        Ok(shortcuts) => shortcuts.check(id, key).await,
        Err(_) => shortcuts::unchecked(id, key),
    }
}

/// Pause global shortcuts while a new key combination is being recorded (KDE only).
pub async fn block_shortcuts(on: bool) {
    if let Ok(shortcuts) = desktop().await.shortcuts {
        shortcuts.block(on).await
    }
}

/// Open the desktop's shortcut editor.
pub async fn configure_shortcuts() {
    if let Ok(shortcuts) = desktop().await.shortcuts {
        shortcuts.configure().await
    }
}
