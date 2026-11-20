//! Linux / KDE Plasma (Wayland): XDG desktop portals, data-control clipboard, SNI tray.

mod kwin;
mod portal;
mod tray;

pub use kwin::focus_game;
pub use portal::{configure_shortcuts, copy_item, describe_shortcuts, run_shortcuts};
pub use tray::{set_game as set_tray_game, spawn as spawn_tray};

pub async fn init() {
    portal::register().await;
    tokio::spawn(portal::warm_up_keyboard());
}
