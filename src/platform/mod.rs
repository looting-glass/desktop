//! Platform layer: everything that talks to the desktop. The rest of the app (item parsing,
//! trade/poe.ninja clients, Qt UI) is platform-independent and only calls the functions below.
//!
//! Every backend must provide:
//! - `async fn init()`                         — one-time setup (app registration, warm-ups)
//! - `async fn run_shortcuts(Sender<Event>)`   — register global hotkeys, forward activations
//! - `async fn desktop_info() -> DesktopInfo`  — how the app works with this desktop, for the settings window
//! - `async fn shortcut_list() -> ShortcutList` — current bindings, for the settings window
//! - `async fn check_shortcut(&str, i32)`      — a recorded key's text and who else uses it
//! - `async fn set_shortcut(&str, i32)`        — bind a Qt key combination (KDE; elsewhere an error)
//! - `async fn block_shortcuts(bool)`          — pause global shortcuts while recording keys
//! - `async fn configure_shortcuts()`          — open the desktop's hotkey editor
//! - `fn reload_shortcuts()`                   — config changed; re-register app-owned hotkeys
//! - `fn trigger(&str) -> Result<()>`          — `lootingglass trigger <id>`: hand a shortcut to the running instance
//! - `async fn notify(&str, &str, Option<Event>) -> bool` — desktop notification (false: none shown)
//! - `async fn copy_item() -> Result<String>`  — make the game copy the hovered item, return its text
//! - `async fn focus_game()`                   — give keyboard focus back to the game
//! - `async fn spawn_tray(Game)` / `async fn set_tray_game(Game)` — system tray icon
//! - `async fn set_tray_update(Option<String>)` — newer version for the tray menu ("Update to …")
//!
//! The overlay window itself is set up in `cpp/main.cpp` (layer-shell on Linux), as is the
//! Windows tray icon (QSystemTrayIcon).

/// How the app does each job on this desktop, for the settings window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DesktopInfo {
    /// "Desktop: Umbriel · Shortcuts: command · Copy: virtual keyboard · …"
    pub summary: String,
    /// What this desktop can't do, in words for the user.
    pub missing: Vec<String>,
}

impl DesktopInfo {
    /// `providers` are (role, provider) pairs, such as ("Copy", "virtual keyboard").
    fn new(desktop: &str, providers: &[(&str, &str)], missing: Vec<String>) -> DesktopInfo {
        let desktop = if desktop.is_empty() { "unknown" } else { desktop };
        let summary = std::iter::once(format!("Desktop: {desktop}"))
            .chain(providers.iter().map(|(role, provider)| format!("{role}: {provider}")))
            .collect::<Vec<_>>()
            .join(" · ");
        DesktopInfo { summary, missing }
    }
}

/// Global shortcuts as the settings window shows them.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ShortcutList {
    /// The keys can be changed in the settings window itself.
    pub editable_here: bool,
    /// How to bind the keys, from the shortcuts provider in use.
    pub bind_help: BindHelp,
    pub rows: Vec<ShortcutRow>,
    pub error: String,
}

/// How to bind our shortcuts on this desktop, in its own terms.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BindHelp {
    /// What to do, in a sentence or two.
    pub text: String,
    /// Lines to add to the window manager's config, one per shortcut; empty when the keys are
    /// set somewhere else.
    pub snippet: String,
    /// The desktop has its own shortcut editor, which `configure_shortcuts` opens.
    pub desktop_editor: bool,
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
