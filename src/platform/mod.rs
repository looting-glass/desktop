//! Platform layer: everything that talks to the desktop. The rest of the app (item parsing,
//! trade/poe.ninja clients, Qt UI) is platform-independent and only calls the functions below.
//!
//! Every backend must provide:
//! - `async fn init()`                         — one-time setup (app registration, warm-ups)
//! - `async fn run_shortcuts(Sender<Event>)`   — register global hotkeys, forward activations
//! - `async fn shortcut_list() -> ShortcutList` — current bindings, for the settings window
//! - `async fn check_shortcut(&str, i32)`      — a recorded key's text and who else uses it
//! - `async fn set_shortcut(&str, i32)`        — bind a Qt key combination (KDE; elsewhere an error)
//! - `async fn block_shortcuts(bool)`          — pause global shortcuts while recording keys
//! - `async fn configure_shortcuts()`          — open the desktop's hotkey editor
//! - `fn reload_shortcuts()`                   — config changed; re-register app-owned hotkeys
//! - `async fn copy_item() -> Result<String>`  — make the game copy the hovered item, return its text
//! - `async fn focus_game()`                   — give keyboard focus back to the game
//! - `async fn spawn_tray(Game)` / `async fn set_tray_game(Game)` — system tray icon
//! - `async fn set_tray_update(Option<String>)` — newer version for the tray menu ("Update to …")
//!
//! The overlay window itself is set up in `cpp/main.cpp` (layer-shell on Linux), as is the
//! Windows tray icon (QSystemTrayIcon).

/// Global shortcuts as the settings window shows them.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ShortcutList {
    /// "kde" (editable here), "hyprland" (bound in hyprland.conf), "portal" (the desktop's own
    /// editor) or "app" (Windows: the app registers them from the config).
    pub mode: &'static str,
    pub rows: Vec<ShortcutRow>,
    pub error: String,
}

/// A key combination recorded in the settings window, before it's saved.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ShortcutCheck {
    pub id: String,
    pub key: i32,
    /// "Ctrl+D", or empty for none.
    pub keys: String,
    /// "org.chromium.Chromium: Electron shortcut Ctrl+D" when another shortcut uses the key.
    pub conflict: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ShortcutRow {
    pub id: String,
    pub description: String,
    /// "Ctrl+D", or empty when unassigned.
    pub keys: String,
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(any(windows, test))]
mod hotkey;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
