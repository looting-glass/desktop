//! Global hotkeys: registered through the GlobalShortcuts portal where there is one (on KDE
//! also listed and edited in our own settings window through kglobalaccel), and elsewhere bound
//! by the user to `lootingglass trigger <id>` commands.

mod command;
mod hyprland_config;
mod kglobalaccel;
mod portal;

use super::facts::Facts;
use super::select::Role;
use crate::Event;
use crate::platform::{BindHelp, ShortcutCheck, ShortcutList, ShortcutRow};
use anyhow::{Result, bail};

/// One of our global shortcuts.
struct ShortcutDef {
    /// The name the desktop and `lootingglass trigger` know it by.
    id: &'static str,
    /// Shown in the desktop's settings.
    description: &'static str,
    /// What it asks the app to do.
    event: Event,
    /// The key the desktop is asked to bind it to, in the portal's notation.
    preferred_trigger: Option<&'static str>,
}

/// Our shortcuts, in settings order.
const SHORTCUT_DEFS: &[ShortcutDef] = &[
    ShortcutDef {
        id: "price-check",
        description: "Price check hovered item",
        event: Event::PriceCheck,
        preferred_trigger: Some("CTRL+D"),
    },
    ShortcutDef {
        id: "item-info",
        description: "Open hovered item on poedb or the wiki",
        event: Event::OpenItemInfo,
        preferred_trigger: None,
    },
    ShortcutDef {
        id: "toggle-game",
        description: "Switch between PoE 1 and PoE 2",
        event: Event::ToggleGame,
        preferred_trigger: None,
    },
    ShortcutDef {
        id: "manual-check",
        description: "Open manual price check",
        event: Event::OpenManual,
        preferred_trigger: None,
    },
    ShortcutDef {
        id: "settings",
        description: "Open Looting Glass settings",
        event: Event::OpenSettings,
        preferred_trigger: None,
    },
    ShortcutDef {
        id: "hide-overlay",
        description: "Hide price overlay",
        event: Event::HideOverlay,
        preferred_trigger: None,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcuts {
    /// KDE: the portal's shortcuts, read and edited here through kglobalaccel (a decorator over
    /// `Portal`).
    KGlobalAccel,
    /// Hyprland: the portal's shortcuts, bound by the user in hyprland.conf.
    HyprlandConfig,
    /// The portal's shortcuts, edited in the desktop's own shortcut settings.
    Portal,
    /// Keys the user binds to `lootingglass trigger <id>` in the window manager's config (Sway
    /// `bindsym`, Umbriel `[keybinds]`). Every window manager can run a command on a key, so
    /// this works wherever there's no portal.
    Command,
}

impl Role for Shortcuts {
    const PRIORITY: &'static [Self] = &[Self::KGlobalAccel, Self::HyprlandConfig, Self::Portal, Self::Command];

    fn available(self, facts: &Facts) -> bool {
        let portal = facts.global_shortcuts_portal.is_some();
        match self {
            // KWin's portal backend is what stores our shortcuts in kglobalaccel.
            Self::KGlobalAccel => portal && facts.kwin && facts.kglobalaccel,
            Self::HyprlandConfig => portal && facts.hyprland_ipc,
            Self::Portal => portal,
            Self::Command => true,
        }
    }
}

/// What the shortcut with this id asks the app to do.
fn event(id: &str) -> Option<Event> {
    find(id).map(|def| def.event.clone())
}

fn find(id: &str) -> Option<&'static ShortcutDef> {
    SHORTCUT_DEFS.iter().find(|def| def.id == id)
}

/// Hand the shortcut `id` to the running instance, for `lootingglass trigger <id>`. Blocking.
pub fn trigger(id: &str) -> Result<()> {
    if event(id).is_none() {
        let ids: Vec<&str> = SHORTCUT_DEFS.iter().map(|def| def.id).collect();
        bail!("Unknown shortcut {id:?}. Use one of: {}.", ids.join(", "));
    }
    command::send(id)
}

impl Shortcuts {
    /// Register the shortcuts and forward their activations until the portal closes.
    pub async fn run(self, tx: async_channel::Sender<Event>) -> Result<()> {
        match self {
            Self::KGlobalAccel => {
                // 0.8/0.9 had a separate map check shortcut; maps are now checked by the price check.
                let _ = kglobalaccel::forget("map-check").await;
                portal::run(tx).await
            }
            Self::HyprlandConfig | Self::Portal => portal::run(tx).await,
            Self::Command => command::run(tx).await,
        }
    }

    /// How the settings window and the docs name it.
    pub fn label(self) -> &'static str {
        match self {
            Self::KGlobalAccel => "KDE shortcuts",
            Self::HyprlandConfig => "hyprland.conf",
            Self::Portal => "portal",
            Self::Command => "command",
        }
    }

    /// How the user binds our shortcuts with this provider.
    fn bind_help(self) -> BindHelp {
        match self {
            Self::KGlobalAccel => kglobalaccel::bind_help(),
            Self::HyprlandConfig => hyprland_config::bind_help(),
            Self::Portal => portal::bind_help(),
            Self::Command => command::bind_help(),
        }
    }

    /// Our shortcuts and their keys, for the settings window.
    pub async fn list(self) -> ShortcutList {
        match self {
            Self::KGlobalAccel => match kglobalaccel::list().await {
                Ok(bound) => ShortcutList {
                    editable_here: true,
                    bind_help: self.bind_help(),
                    rows: rows(|id| {
                        let keys = bound.iter().find(|(i, ..)| i == id).map(|(.., k)| k.as_slice()).unwrap_or_default();
                        keys.iter().map(|k| kglobalaccel::key_text(*k)).collect::<Vec<_>>().join(", ")
                    }),
                    error: String::new(),
                },
                Err(e) => {
                    log::info!("kglobalaccel unavailable, using the portal's shortcut editor: {e:#}");
                    portal_list(Self::Portal).await
                }
            },
            Self::HyprlandConfig | Self::Portal => portal_list(self).await,
            // The keys live in the window manager's config, which we don't read.
            Self::Command => ShortcutList { bind_help: self.bind_help(), rows: rows(|_| String::new()), ..Default::default() },
        }
    }

    /// Bind `key` (a Qt key combination, 0 = none) to shortcut `id`, taking it from another
    /// shortcut that uses it.
    pub async fn set(self, id: &str, key: i32) -> Result<()> {
        if self != Self::KGlobalAccel {
            bail!("This desktop's shortcuts can't be changed from Looting Glass.");
        }
        let Some(def) = find(id) else {
            bail!("unknown shortcut {id}");
        };
        kglobalaccel::set(id, def.description, key, true).await
    }

    /// How `key` reads ("Ctrl+D") and who else uses it, before it's saved.
    pub async fn check(self, id: &str, key: i32) -> ShortcutCheck {
        if self != Self::KGlobalAccel {
            return unchecked(id, key);
        }
        let conflict = match kglobalaccel::conflict(id, key).await {
            Ok(c) => c.unwrap_or_default(),
            Err(e) => format!("Couldn't check the key: {e:#}"),
        };
        ShortcutCheck {
            id: id.into(),
            key,
            keys: if key == 0 { String::new() } else { kglobalaccel::key_text(key) },
            conflict,
        }
    }

    /// Pause global shortcuts while a new key combination is being recorded.
    pub async fn block(self, on: bool) {
        if self == Self::KGlobalAccel
            && let Err(e) = kglobalaccel::block(on).await
        {
            log::warn!("kglobalaccel blockGlobalShortcuts: {e:#}");
        }
    }

    /// Open the desktop's shortcut editor.
    pub async fn configure(self) {
        if !portal::configure().await && self == Self::KGlobalAccel {
            let _ = std::process::Command::new("systemsettings").arg("kcm_keys").spawn();
        }
    }
}

/// A recorded key, passed back as it is where nothing can check it.
fn unchecked(id: &str, key: i32) -> ShortcutCheck {
    ShortcutCheck { id: id.into(), key, ..Default::default() }
}

/// One row per shortcut, in settings order, with the keys `keys_of` gives for its id.
fn rows(keys_of: impl Fn(&str) -> String) -> Vec<ShortcutRow> {
    SHORTCUT_DEFS
        .iter()
        .map(|def| ShortcutRow { id: def.id.into(), description: def.description.into(), keys: keys_of(def.id) })
        .collect()
}

/// Config lines for our shortcuts, one per shortcut in settings order, each with its
/// description as a comment.
fn snippet(line: impl Fn(&ShortcutDef) -> String) -> String {
    SHORTCUT_DEFS.iter().map(|def| format!("{}   # {}", line(def), def.description)).collect::<Vec<_>>().join("\n")
}

/// The portal's shortcuts and their keys, explained the way `provider` binds them.
async fn portal_list(provider: Shortcuts) -> ShortcutList {
    let bind_help = provider.bind_help();
    match portal::triggers().await {
        Ok(triggers) => ShortcutList {
            bind_help,
            rows: rows(|id| triggers.get(id).cloned().unwrap_or_default()),
            ..Default::default()
        },
        Err(e) => ShortcutList { bind_help, rows: rows(|_| String::new()), error: format!("{e:#}"), ..Default::default() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let ids: std::collections::HashSet<_> = SHORTCUT_DEFS.iter().map(|def| def.id).collect();
        assert_eq!(ids.len(), SHORTCUT_DEFS.len());
    }

    #[test]
    fn known_ids_have_their_event() {
        assert!(matches!(event("price-check"), Some(Event::PriceCheck)));
        assert!(matches!(event("hide-overlay"), Some(Event::HideOverlay)));
    }

    #[test]
    fn unknown_ids_have_none() {
        for id in ["", "map-check", "Price-Check", "price-check\n"] {
            assert!(event(id).is_none(), "{id:?}");
        }
    }

    #[test]
    fn trigger_rejects_unknown_ids_before_connecting() {
        let e = trigger("pricecheck").unwrap_err().to_string();
        assert!(e.starts_with("Unknown shortcut \"pricecheck\". Use one of: price-check, item-info,"), "{e}");
    }
}
