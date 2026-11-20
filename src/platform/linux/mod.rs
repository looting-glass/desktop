//! Linux (Wayland): XDG desktop portals, data-control clipboard, SNI tray.
//! Desktop-specific parts (sending Ctrl+C, refocusing the game) differ between KDE Plasma
//! (RemoteDesktop portal + KWin script) and Hyprland (hyprctl dispatchers).

mod hyprland;
mod kwin;
mod portal;
mod tray;

pub use portal::{configure_shortcuts, copy_item, describe_shortcuts, run_shortcuts};
pub use tray::{set_game as set_tray_game, spawn as spawn_tray};

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
