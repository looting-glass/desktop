//! `Facts` of the desktops we know, as their sessions probe, for the selection tests and the
//! docs table (`desktops`).

use super::facts::Facts;

pub const LAYER_SHELL: &str = "zwlr_layer_shell_v1";
pub const EXT_DATA_CONTROL: &str = "ext_data_control_manager_v1";
pub const WLR_DATA_CONTROL: &str = "zwlr_data_control_manager_v1";
pub const VIRTUAL_KEYBOARD: &str = "zwp_virtual_keyboard_manager_v1";

pub fn facts(desktop: &str, globals: &[&str]) -> Facts {
    Facts {
        desktop: desktop.into(),
        wayland_globals: globals.iter().map(|g| g.to_string()).collect(),
        ..Facts::default()
    }
}

/// Plasma 6, as probed on a real session (ext data-control only, no wlr).
pub fn kde() -> Facts {
    Facts {
        global_shortcuts_portal: Some(2),
        remote_desktop_portal: Some(2),
        kwin: true,
        kglobalaccel: true,
        ..facts("KDE", &[LAYER_SHELL, EXT_DATA_CONTROL])
    }
}

/// xdg-desktop-portal-hyprland: GlobalShortcuts, but no RemoteDesktop.
pub fn hyprland() -> Facts {
    Facts {
        global_shortcuts_portal: Some(1),
        hyprland_ipc: true,
        ..facts("Hyprland", &[LAYER_SHELL, EXT_DATA_CONTROL, WLR_DATA_CONTROL, VIRTUAL_KEYBOARD])
    }
}

/// A wlroots compositor with xdg-desktop-portal-wlr (neither GlobalShortcuts nor RemoteDesktop)
/// and no IPC we use: river, Wayfire.
pub fn wlroots() -> Facts {
    facts("", &[LAYER_SHELL, EXT_DATA_CONTROL, WLR_DATA_CONTROL, VIRTUAL_KEYBOARD])
}

pub fn sway() -> Facts {
    Facts { desktop: "sway".into(), sway_ipc: true, ..wlroots() }
}

/// Umbriel's docs don't say whether its shortcut bridge is the GlobalShortcuts portal.
pub fn umbriel() -> Facts {
    Facts { desktop: "Umbriel".into(), umbriel_ipc: true, ..wlroots() }
}

/// Mutter has no layer-shell or data-control; GlobalShortcuts arrived in GNOME 48.
pub fn gnome_47() -> Facts {
    Facts { remote_desktop_portal: Some(2), ..facts("GNOME", &[]) }
}

pub fn gnome_48() -> Facts {
    Facts { global_shortcuts_portal: Some(1), ..gnome_47() }
}

/// A compositor with only the core protocols and no portals (cage, weston).
pub fn bare() -> Facts {
    facts("", &[])
}
