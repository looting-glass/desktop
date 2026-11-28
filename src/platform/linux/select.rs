//! Which provider does each job on this desktop, decided from its `Facts` alone.

use super::clipboard::Clipboard;
use super::facts::Facts;
use super::focus::Focus;
use super::keys::KeySender;
use super::shortcuts::Shortcuts;
use crate::platform::DesktopInfo;

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
    /// Every desktop can at least run `lootingglass trigger` on a key (`Shortcuts::Command`).
    pub shortcuts: Shortcuts,
    pub keys: Result<KeySender, Missing>,
    /// Refocusing the game only helps, so a desktop without a way to do it gets `Focus::None`.
    pub focus: Focus,
    pub clipboard: Result<Clipboard, Missing>,
}

impl Selection {
    /// What the settings window says about `desktop` (its `Facts::desktop` label). The clipboard
    /// has one provider, so only its absence is worth a word.
    pub fn describe(&self, desktop: &str) -> DesktopInfo {
        let providers = [
            ("Shortcuts", self.shortcuts.label()),
            ("Copy", self.keys.map_or("none", KeySender::label)),
            ("Focus", self.focus.label()),
        ];
        let missing = [self.keys.err(), self.clipboard.err()].into_iter().flatten().map(|m| m.to_string()).collect();
        DesktopInfo::new(desktop, &providers, missing)
    }
}

pub fn select(facts: &Facts) -> Selection {
    Selection {
        shortcuts: pick(facts).unwrap_or(Shortcuts::Command),
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
        Facts { sway_ipc: true, ..facts("sway", &[LAYER_SHELL, EXT_DATA_CONTROL, WLR_DATA_CONTROL, VIRTUAL_KEYBOARD]) }
    }

    /// Umbriel's docs don't say whether its shortcut bridge is the GlobalShortcuts portal.
    fn umbriel() -> Facts {
        Facts {
            umbriel_ipc: true,
            ..facts("Umbriel", &[LAYER_SHELL, EXT_DATA_CONTROL, WLR_DATA_CONTROL, VIRTUAL_KEYBOARD])
        }
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
                shortcuts: Shortcuts::KGlobalAccel,
                keys: Ok(KeySender::RemoteDesktop),
                focus: Focus::KWinScript,
                clipboard: Ok(Clipboard::DataControl),
            }
        );
    }

    #[test]
    fn kde_without_kglobalaccel_uses_the_portal_editor() {
        let facts = Facts { kglobalaccel: false, ..kde() };
        assert_eq!(select(&facts).shortcuts, Shortcuts::Portal);
    }

    #[test]
    fn hyprland_drives_the_game_through_hyprctl() {
        assert_eq!(
            select(&hyprland()),
            Selection {
                shortcuts: Shortcuts::HyprlandConfig,
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
    fn sway_takes_commands_and_types_on_a_virtual_keyboard() {
        assert_eq!(
            select(&sway()),
            Selection {
                shortcuts: Shortcuts::Command,
                keys: Ok(KeySender::VirtualKeyboard),
                focus: Focus::SwayIpc,
                clipboard: Ok(Clipboard::DataControl),
            }
        );
    }

    #[test]
    fn umbriel_takes_commands_and_types_on_a_virtual_keyboard() {
        assert_eq!(
            select(&umbriel()),
            Selection {
                shortcuts: Shortcuts::Command,
                keys: Ok(KeySender::VirtualKeyboard),
                focus: Focus::UmbrielIpc,
                clipboard: Ok(Clipboard::DataControl),
            }
        );
    }

    #[test]
    fn a_global_shortcuts_portal_beats_commands() {
        let facts = Facts { global_shortcuts_portal: Some(1), ..umbriel() };
        assert_eq!(select(&facts).shortcuts, Shortcuts::Portal);
    }

    #[test]
    fn a_remote_desktop_portal_beats_the_virtual_keyboard() {
        // A KDE session that also lists virtual keyboards keeps the tested portal path.
        let facts = Facts { wayland_globals: [LAYER_SHELL, EXT_DATA_CONTROL, VIRTUAL_KEYBOARD].map(String::from).into(), ..kde() };
        assert_eq!(select(&facts).keys, Ok(KeySender::RemoteDesktop));
    }

    #[test]
    fn a_wlroots_compositor_without_ipc_still_copies_but_cant_refocus() {
        // river, Wayfire: the user clicks the game themselves after the overlay closes.
        let facts = Facts { sway_ipc: false, ..sway() };
        let selection = select(&facts);
        assert_eq!((selection.keys, selection.focus), (Ok(KeySender::VirtualKeyboard), Focus::None));
    }

    #[test]
    fn gnome_47_can_send_keys_but_not_read_the_clipboard() {
        assert_eq!(
            select(&gnome_47()),
            Selection {
                shortcuts: Shortcuts::Command,
                keys: Ok(KeySender::RemoteDesktop),
                focus: Focus::None,
                clipboard: Err(Missing::CLIPBOARD),
            }
        );
    }

    #[test]
    fn gnome_48_adds_portal_shortcuts() {
        assert_eq!(select(&gnome_48()).shortcuts, Shortcuts::Portal);
    }

    #[test]
    fn bare_compositor_only_takes_commands() {
        assert_eq!(
            select(&bare()),
            Selection {
                shortcuts: Shortcuts::Command,
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

    fn describe(facts: &Facts) -> DesktopInfo {
        select(facts).describe(&facts.desktop)
    }

    #[test]
    fn kde_is_described_by_its_providers() {
        assert_eq!(
            describe(&kde()),
            DesktopInfo {
                summary: "Desktop: KDE · Shortcuts: KDE shortcuts · Copy: remote desktop portal · Focus: KWin script".into(),
                missing: vec![],
            }
        );
    }

    #[test]
    fn hyprland_is_described_by_its_providers() {
        assert_eq!(
            describe(&hyprland()),
            DesktopInfo {
                summary: "Desktop: Hyprland · Shortcuts: hyprland.conf · Copy: hyprctl · Focus: hyprctl".into(),
                missing: vec![],
            }
        );
    }

    #[test]
    fn sway_is_described_by_its_providers() {
        assert_eq!(
            describe(&sway()),
            DesktopInfo {
                summary: "Desktop: sway · Shortcuts: command · Copy: virtual keyboard · Focus: Sway IPC".into(),
                missing: vec![],
            }
        );
    }

    #[test]
    fn umbriel_is_described_by_its_providers() {
        assert_eq!(
            describe(&umbriel()),
            DesktopInfo {
                summary: "Desktop: Umbriel · Shortcuts: command · Copy: virtual keyboard · Focus: Umbriel IPC".into(),
                missing: vec![],
            }
        );
    }

    #[test]
    fn a_bare_compositor_says_what_it_cant_do() {
        assert_eq!(
            describe(&bare()),
            DesktopInfo {
                summary: "Desktop: unknown · Shortcuts: command · Copy: none · Focus: none".into(),
                missing: vec![Missing::KEYS.to_string(), Missing::CLIPBOARD.to_string()],
            }
        );
    }
}
