//! Global hotkeys: registered through the GlobalShortcuts portal where there is one (on KDE
//! also listed and edited in our own settings window through kglobalaccel), and elsewhere bound
//! by the user to `lootingglass trigger <id>` commands.

mod command;
mod kglobalaccel;
mod portal;

use super::facts::Facts;
use super::select::Role;
use crate::Event;
use crate::platform::{ShortcutCheck, ShortcutList, ShortcutRow};
use anyhow::{Result, bail};

/// Shortcut id, description (shown in the desktop's settings), preferred trigger; in settings order.
const SHORTCUT_DEFS: &[(&str, &str, Option<&str>)] = &[
    ("price-check", "Price check hovered item", Some("CTRL+D")),
    ("item-info", "Open hovered item on poedb or the wiki", None),
    ("toggle-game", "Switch between PoE 1 and PoE 2", None),
    ("manual-check", "Open manual price check", None),
    ("settings", "Open Looting Glass settings", None),
    ("hide-overlay", "Hide price overlay", None),
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
    Some(match id {
        "price-check" => Event::PriceCheck,
        "toggle-game" => Event::ToggleGame,
        "settings" => Event::OpenSettings,
        "manual-check" => Event::OpenManual,
        "hide-overlay" => Event::HideOverlay,
        "item-info" => Event::OpenItemInfo,
        _ => return None,
    })
}

/// Hand the shortcut `id` to the running instance, for `lootingglass trigger <id>`. Blocking.
pub fn trigger(id: &str) -> Result<()> {
    if event(id).is_none() {
        let ids: Vec<&str> = SHORTCUT_DEFS.iter().map(|(id, ..)| *id).collect();
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

    /// Our shortcuts and their keys, for the settings window.
    pub async fn list(self) -> ShortcutList {
        match self {
            Self::KGlobalAccel => match kglobalaccel::list().await {
                Ok(bound) => ShortcutList {
                    mode: "kde",
                    rows: rows(|id| {
                        let keys = bound.iter().find(|(i, ..)| i == id).map(|(.., k)| k.as_slice()).unwrap_or_default();
                        keys.iter().map(|k| kglobalaccel::key_text(*k)).collect::<Vec<_>>().join(", ")
                    }),
                    error: String::new(),
                },
                Err(e) => {
                    log::info!("kglobalaccel unavailable, using the portal's shortcut editor: {e:#}");
                    portal_list("portal").await
                }
            },
            Self::HyprlandConfig => portal_list("hyprland").await,
            Self::Portal => portal_list("portal").await,
            // The keys live in the window manager's config, which we don't read.
            Self::Command => ShortcutList { mode: "command", rows: rows(|_| String::new()), error: String::new() },
        }
    }

    /// Bind `key` (a Qt key combination, 0 = none) to shortcut `id`, taking it from another
    /// shortcut that uses it.
    pub async fn set(self, id: &str, key: i32) -> Result<()> {
        if self != Self::KGlobalAccel {
            bail!("This desktop's shortcuts can't be changed from Looting Glass.");
        }
        let Some((_, description, _)) = SHORTCUT_DEFS.iter().find(|(i, ..)| *i == id) else {
            bail!("unknown shortcut {id}");
        };
        kglobalaccel::set(id, description, key, true).await
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
        .map(|(id, description, _)| ShortcutRow { id: (*id).into(), description: (*description).into(), keys: keys_of(id) })
        .collect()
}

async fn portal_list(mode: &'static str) -> ShortcutList {
    match portal::triggers().await {
        Ok(triggers) => ShortcutList {
            mode,
            rows: rows(|id| triggers.get(id).cloned().unwrap_or_default()),
            error: String::new(),
        },
        Err(e) => ShortcutList { mode, rows: rows(|_| String::new()), error: format!("{e:#}") },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shortcut_has_an_event() {
        for (id, ..) in SHORTCUT_DEFS {
            assert!(event(id).is_some(), "{id}");
        }
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
