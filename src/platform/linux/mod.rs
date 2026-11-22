//! Linux (Wayland): XDG desktop portals, data-control clipboard, SNI tray.
//! Desktop-specific parts (sending Ctrl+C, refocusing the game) differ between KDE Plasma
//! (RemoteDesktop portal + KWin script) and Hyprland (hyprctl dispatchers).

mod hyprland;
mod kglobalaccel;
mod kwin;
mod portal;
mod tray;

use super::{ShortcutList, ShortcutRow};
pub use portal::{configure_shortcuts, copy_item, run_shortcuts};
pub use tray::{set_game as set_tray_game, set_update as set_tray_update, spawn as spawn_tray};

pub async fn init() {
    portal::register().await;
    if !hyprland::detected() {
        tokio::spawn(portal::warm_up_keyboard());
    }
}

/// Give keyboard focus back to the game (after the overlay closes, before copying).
pub async fn focus_game() {
    if hyprland::detected() {
        hyprland::focus_game().await
    } else {
        kwin::focus_game().await
    }
}

/// Shortcuts are bound in the desktop's settings (portal), nothing to reload.
pub fn reload_shortcuts() {}

/// Our global shortcuts for the settings window. On KDE they're read from (and editable through)
/// kglobalaccel; Hyprland binds them in hyprland.conf; other portals have their own editor.
pub async fn shortcut_list() -> ShortcutList {
    let row = |id: &str, description: &str, keys: String| ShortcutRow { id: id.into(), description: description.into(), keys };
    if !hyprland::detected() {
        match kglobalaccel::list().await {
            Ok(list) => {
                let rows = portal::SHORTCUT_DEFS
                    .iter()
                    .map(|(id, desc, _)| {
                        let keys = list.iter().find(|(i, ..)| i == id).map(|(.., k)| k.as_slice()).unwrap_or_default();
                        row(id, desc, keys.iter().map(|k| kglobalaccel::key_text(*k)).collect::<Vec<_>>().join(", "))
                    })
                    .collect();
                return ShortcutList { mode: "kde", rows, error: String::new() };
            }
            Err(e) => log::info!("kglobalaccel unavailable, using the portal's shortcut editor: {e:#}"),
        }
    }
    let (triggers, error) = match portal::triggers().await {
        Ok(t) => (t, String::new()),
        Err(e) => (Default::default(), format!("{e:#}")),
    };
    let rows = portal::SHORTCUT_DEFS
        .iter()
        .map(|(id, desc, _)| row(id, desc, triggers.get(*id).cloned().unwrap_or_default()))
        .collect();
    ShortcutList { mode: if hyprland::detected() { "hyprland" } else { "portal" }, rows, error }
}

/// Bind `key` (a Qt key combination, 0 = none) to shortcut `id`, taking it from another
/// shortcut that uses it (KDE only).
pub async fn set_shortcut(id: &str, key: i32) -> anyhow::Result<()> {
    let Some((_, desc, _)) = portal::SHORTCUT_DEFS.iter().find(|(i, ..)| *i == id) else {
        anyhow::bail!("unknown shortcut {id}");
    };
    kglobalaccel::set(id, desc, key, true).await
}

/// How `key` reads ("Ctrl+D") and who else uses it, before it's saved (KDE only).
pub async fn check_shortcut(id: &str, key: i32) -> super::ShortcutCheck {
    let conflict = match kglobalaccel::conflict(id, key).await {
        Ok(c) => c.unwrap_or_default(),
        Err(e) => format!("Couldn't check the key: {e:#}"),
    };
    super::ShortcutCheck {
        id: id.into(),
        key,
        keys: if key == 0 { String::new() } else { kglobalaccel::key_text(key) },
        conflict,
    }
}

/// Pause global shortcuts while a new key combination is being recorded (KDE only).
pub async fn block_shortcuts(on: bool) {
    if !hyprland::detected()
        && let Err(e) = kglobalaccel::block(on).await
    {
        log::warn!("kglobalaccel blockGlobalShortcuts: {e:#}");
    }
}
