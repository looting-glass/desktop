//! Which provider does each job on this desktop, decided from its `Facts` alone.

use super::clipboard::Clipboard;
use super::facts::Facts;
use super::focus::Focus;
use super::keys::KeySender;
use super::shortcuts::Shortcuts;

/// A job the app needs from the desktop, done by one of several interchangeable providers.
pub trait Role: Copy + 'static {
    /// Every provider of the role, best first.
    const PRIORITY: &'static [Self];
    /// Whether the desktop offers what this provider needs.
    fn available(self, facts: &Facts) -> bool;
}

/// The first provider of the role that the desktop supports.
fn pick<R: Role>(facts: &Facts) -> Option<R> {
    R::PRIORITY.iter().copied().find(|provider| provider.available(facts))
}

/// Why a role has no provider on this desktop, in words for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missing(&'static str);

impl Missing {
    pub const SHORTCUTS: Missing =
        Missing("This desktop has no global shortcuts portal, so Looting Glass can't register hotkeys.");
    pub const KEYS: Missing = Missing(
        "This desktop can't send keys to the game, so the hotkey price check is unavailable. Use the manual check.",
    );
    pub const CLIPBOARD: Missing = Missing(
        "This desktop doesn't let apps read the clipboard in the background, so the hotkey price check is unavailable. Use the manual check.",
    );
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Missing {}

/// The provider chosen for each role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub shortcuts: Result<Shortcuts, Missing>,
    pub keys: Result<KeySender, Missing>,
    /// Refocusing the game only helps, so a desktop without a way to do it gets `Focus::None`.
    pub focus: Focus,
    pub clipboard: Result<Clipboard, Missing>,
}

pub fn select(facts: &Facts) -> Selection {
    Selection {
        shortcuts: pick(facts).ok_or(Missing::SHORTCUTS),
        keys: pick(facts).ok_or(Missing::KEYS),
        focus: pick(facts).unwrap_or(Focus::None),
        clipboard: pick(facts).ok_or(Missing::CLIPBOARD),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYER_SHELL: &str = "zwlr_layer_shell_v1";
    const EXT_DATA_CONTROL: &str = "ext_data_control_manager_v1";
    const WLR_DATA_CONTROL: &str = "zwlr_data_control_manager_v1";
    const VIRTUAL_KEYBOARD: &str = "zwp_virtual_keyboard_manager_v1";

    fn facts(desktop: &str, globals: &[&str]) -> Facts {
        Facts {
            desktop: desktop.into(),
            wayland_globals: globals.iter().map(|g| g.to_string()).collect(),
            ..Facts::default()
        }
    }

    /// Plasma 6, as probed on a real session (ext data-control only, no wlr).
    fn kde() -> Facts {
        Facts {
            global_shortcuts_portal: Some(2),
            remote_desktop_portal: Some(2),
            kwin: true,
            kglobalaccel: true,
            ..facts("KDE", &[LAYER_SHELL, EXT_DATA_CONTROL])
        }
    }

    /// xdg-desktop-portal-hyprland: GlobalShortcuts, but no RemoteDesktop.
    fn hyprland() -> Facts {
        Facts {
            global_shortcuts_portal: Some(1),
            hyprland_ipc: true,
            ..facts("Hyprland", &[LAYER_SHELL, EXT_DATA_CONTROL, WLR_DATA_CONTROL, VIRTUAL_KEYBOARD])
        }
    }

    /// xdg-desktop-portal-wlr: neither GlobalShortcuts nor RemoteDesktop.
    fn sway() -> Facts {
        facts("sway", &[LAYER_SHELL, EXT_DATA_CONTROL, WLR_DATA_CONTROL, VIRTUAL_KEYBOARD])
    }

    fn umbriel() -> Facts {
        facts("Umbriel", &[LAYER_SHELL, EXT_DATA_CONTROL, WLR_DATA_CONTROL, VIRTUAL_KEYBOARD])
    }

    /// Mutter has no layer-shell or data-control; GlobalShortcuts arrived in GNOME 48.
    fn gnome_47() -> Facts {
        Facts { remote_desktop_portal: Some(2), ..facts("GNOME", &[]) }
    }

    fn gnome_48() -> Facts {
        Facts { global_shortcuts_portal: Some(1), ..gnome_47() }
    }

    /// A compositor with only the core protocols and no portals (cage, weston).
    fn bare() -> Facts {
        facts("", &[])
    }

    #[test]
    fn kde_edits_shortcuts_here_and_copies_through_the_portal() {
        assert_eq!(
            select(&kde()),
            Selection {
                shortcuts: Ok(Shortcuts::KGlobalAccel),
                keys: Ok(KeySender::RemoteDesktop),
                focus: Focus::KWinScript,
                clipboard: Ok(Clipboard::DataControl),
            }
        );
    }

    #[test]
    fn kde_without_kglobalaccel_uses_the_portal_editor() {
        let facts = Facts { kglobalaccel: false, ..kde() };
        assert_eq!(select(&facts).shortcuts, Ok(Shortcuts::Portal));
    }

    #[test]
    fn hyprland_drives_the_game_through_hyprctl() {
        assert_eq!(
            select(&hyprland()),
            Selection {
                shortcuts: Ok(Shortcuts::HyprlandConfig),
                keys: Ok(KeySender::Hyprctl),
                focus: Focus::Hyprctl,
                clipboard: Ok(Clipboard::DataControl),
            }
        );
    }

    #[test]
    fn hyprland_prefers_hyprctl_over_a_remote_desktop_portal() {
        // A portals.conf can route RemoteDesktop to another backend (e.g. GNOME's); hyprctl
        // still sends the keys straight to the game window.
        let facts = Facts { remote_desktop_portal: Some(2), ..hyprland() };
        assert_eq!(select(&facts).keys, Ok(KeySender::Hyprctl));
    }

    #[test]
    fn sway_and_umbriel_only_have_the_clipboard_so_far() {
        for facts in [sway(), umbriel()] {
            assert_eq!(
                select(&facts),
                Selection {
                    shortcuts: Err(Missing::SHORTCUTS),
                    keys: Err(Missing::KEYS),
                    focus: Focus::None,
                    clipboard: Ok(Clipboard::DataControl),
                },
                "{}",
                facts.desktop
            );
        }
    }

    #[test]
    fn gnome_47_can_send_keys_but_not_read_the_clipboard() {
        assert_eq!(
            select(&gnome_47()),
            Selection {
                shortcuts: Err(Missing::SHORTCUTS),
                keys: Ok(KeySender::RemoteDesktop),
                focus: Focus::None,
                clipboard: Err(Missing::CLIPBOARD),
            }
        );
    }

    #[test]
    fn gnome_48_adds_portal_shortcuts() {
        assert_eq!(select(&gnome_48()).shortcuts, Ok(Shortcuts::Portal));
    }

    #[test]
    fn bare_compositor_has_nothing() {
        assert_eq!(
            select(&bare()),
            Selection {
                shortcuts: Err(Missing::SHORTCUTS),
                keys: Err(Missing::KEYS),
                focus: Focus::None,
                clipboard: Err(Missing::CLIPBOARD),
            }
        );
    }

    #[test]
    fn either_data_control_protocol_reads_the_clipboard() {
        for global in [EXT_DATA_CONTROL, WLR_DATA_CONTROL] {
            assert_eq!(select(&facts("", &[global])).clipboard, Ok(Clipboard::DataControl), "{global}");
        }
    }
}
