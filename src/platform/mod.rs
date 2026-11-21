//! Platform layer: everything that talks to the desktop. The rest of the app (item parsing,
//! trade/poe.ninja clients, Qt UI) is platform-independent and only calls the functions below.
//!
//! Every backend must provide:
//! - `async fn init()`                         — one-time setup (app registration, warm-ups)
//! - `async fn run_shortcuts(Sender<Event>)`   — register global hotkeys, forward activations
//! - `async fn describe_shortcuts() -> String` — current bindings, for the settings window
//! - `async fn configure_shortcuts()`          — open the hotkey editor
//! - `fn reload_shortcuts()`                   — config changed; re-register app-owned hotkeys
//! - `async fn copy_item() -> Result<String>`  — make the game copy the hovered item, return its text
//! - `async fn focus_game()`                   — give keyboard focus back to the game
//! - `async fn spawn_tray(Game)` / `async fn set_tray_game(Game)` — system tray icon
//! - `async fn set_tray_update(Option<String>)` — newer version for the tray menu ("Update to …")
//!
//! The overlay window itself is set up in `cpp/main.cpp` (layer-shell on Linux), as is the
//! Windows tray icon (QSystemTrayIcon).

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
