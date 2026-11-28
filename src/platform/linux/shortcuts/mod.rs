//! Global hotkeys: registered through the GlobalShortcuts portal everywhere, and on KDE also
//! listed and edited in our own settings window through kglobalaccel.

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
}

impl Role for Shortcuts {
    const PRIORITY: &'static [Self] = &[Self::KGlobalAccel, Self::HyprlandConfig, Self::Portal];

    fn available(self, facts: &Facts) -> bool {
        facts.global_shortcuts_portal.is_some()
            && match self {
                // KWin's portal backend is what stores our shortcuts in kglobalaccel.
                Self::KGlobalAccel => facts.kwin && facts.kglobalaccel,
                Self::HyprlandConfig => facts.hyprland_ipc,
                Self::Portal => true,
            }
    }
}

impl Shortcuts {
    /// Register the shortcuts and forward their activations until the portal closes.
    pub async fn run(self, tx: async_channel::Sender<Event>) -> Result<()> {
        if self == Self::KGlobalAccel {
            // 0.8/0.9 had a separate map check shortcut; maps are now checked by the price check.
            let _ = kglobalaccel::forget("map-check").await;
        }
        portal::run(tx).await
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
pub fn unchecked(id: &str, key: i32) -> ShortcutCheck {
    ShortcutCheck { id: id.into(), key, ..Default::default() }
}

/// One row per shortcut, in settings order, with the keys `keys_of` gives for its id.
pub fn rows(keys_of: impl Fn(&str) -> String) -> Vec<ShortcutRow> {
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
