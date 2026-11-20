//! Windows backend — not implemented yet. Planned approach:
//! - hotkeys:   RegisterHotKey (e.g. the `global-hotkey` crate); bindings stored in config.toml
//!              and edited in Loot Lens's own settings, since Windows has no shortcut portal
//! - copy_item: SendInput Ctrl+C (e.g. `enigo`), then read the Win32 clipboard (e.g. `arboard`)
//! - tray:      QSystemTrayIcon (could replace ksni on Linux too)
//! - overlay:   frameless always-on-top Qt::Tool window (see cpp/main.cpp); PoE must run in
//!              windowed fullscreen, exclusive fullscreen can't be overlaid

use crate::Event;
use crate::config::Game;
use anyhow::{Result, bail};

pub async fn init() {}

pub async fn run_shortcuts(_tx: async_channel::Sender<Event>) -> Result<()> {
    bail!("Global hotkeys are not implemented on Windows yet")
}

pub async fn describe_shortcuts() -> String {
    "Hotkeys are not implemented on Windows yet".into()
}

pub async fn configure_shortcuts() {}

pub async fn copy_item() -> Result<String> {
    bail!("Copying items is not implemented on Windows yet")
}

pub async fn spawn_tray(_game: Game) {}

pub async fn set_tray_game(_game: Game) {}

/// Return keyboard focus to the game after the overlay closes (SetForegroundWindow).
pub async fn focus_game() {}
