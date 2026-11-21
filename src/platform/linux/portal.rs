//! Wayland-native integration through XDG desktop portals:
//! - GlobalShortcuts: hotkeys that work while the game has focus (configured in KDE System Settings)
//! - RemoteDesktop (keyboard only): sends Ctrl+C to the game so it copies the hovered item
//! - wlr/ext data-control (via wl-clipboard-rs): reads the clipboard without having focus

use crate::Event;
use anyhow::{Context, Result};
use ashpd::desktop::{
    PersistMode, Session,
    global_shortcuts::{GlobalShortcuts, NewShortcut},
    remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions},
};
use futures_util::StreamExt;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{Mutex, OnceCell};

pub const APP_ID: &str = "dev.lootingglass.LootingGlass";

const KEY_LEFTCTRL: i32 = 29;
const KEY_C: i32 = 46;

struct Shortcuts {
    gs: GlobalShortcuts,
    session: Session<GlobalShortcuts>,
}
static SHORTCUTS: OnceCell<Shortcuts> = OnceCell::const_new();

struct Keyboard {
    rd: RemoteDesktop,
    session: Session<RemoteDesktop>,
}
static KEYBOARD: Mutex<Option<Keyboard>> = Mutex::const_new(None);

/// Tell the portal which .desktop file we are (needed for unsandboxed apps).
pub async fn register() {
    match APP_ID.try_into() {
        Ok(id) => {
            if let Err(e) = ashpd::register_host_app(id).await {
                log::warn!("portal host registration failed (old xdg-desktop-portal?): {e}");
            }
        }
        Err(e) => log::warn!("bad app id: {e}"),
    }
}

pub async fn run_shortcuts(tx: async_channel::Sender<Event>) -> Result<()> {
    let sc = SHORTCUTS
        .get_or_try_init(|| async {
            let gs = GlobalShortcuts::new().await?;
            let session = gs.create_session(Default::default()).await?;
            let shortcuts = [
                NewShortcut::new("price-check", "Price check hovered item").preferred_trigger("CTRL+D"),
                NewShortcut::new("toggle-game", "Switch between PoE 1 and PoE 2").preferred_trigger(None),
                NewShortcut::new("settings", "Open Looting Glass settings").preferred_trigger(None),
                NewShortcut::new("manual-check", "Open manual price check").preferred_trigger(None),
                NewShortcut::new("hide-overlay", "Hide price overlay").preferred_trigger(None),
                NewShortcut::new("item-info", "Open hovered item on the wiki or poedb").preferred_trigger(None),
            ];
            gs.bind_shortcuts(&session, &shortcuts, None, Default::default())
                .await?
                .response()?;
            anyhow::Ok(Shortcuts { gs, session })
        })
        .await
        .context("GlobalShortcuts portal unavailable")?;

    let mut stream = sc.gs.receive_activated().await?;
    while let Some(a) = stream.next().await {
        let ev = match a.shortcut_id() {
            "price-check" => Event::PriceCheck,
            "toggle-game" => Event::ToggleGame,
            "settings" => Event::OpenSettings,
            "manual-check" => Event::OpenManual,
            "hide-overlay" => Event::HideOverlay,
            "item-info" => Event::OpenItemInfo,
            _ => continue,
        };
        let _ = tx.send(ev).await;
    }
    Ok(())
}

/// Human readable list of current bindings, e.g. "Price check: Ctrl+D".
pub async fn describe_shortcuts() -> String {
    let Some(sc) = SHORTCUTS.get() else { return "Global shortcuts portal not connected".into() };
    let res = async {
        let list = sc.gs.list_shortcuts(&sc.session, Default::default()).await?.response()?;
        anyhow::Ok(
            list.shortcuts()
                .iter()
                .map(|s| {
                    let t = s.trigger_description();
                    format!("{}: {}", s.description(), if t.is_empty() { "unassigned" } else { t })
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
    .await;
    let text = res.unwrap_or_else(|e| format!("Could not list shortcuts: {e}"));
    if super::hyprland::detected() {
        format!("{text}\n\nOn Hyprland, bind keys in hyprland.conf, e.g.\n{}", super::hyprland::BIND_HINT)
    } else {
        text
    }
}

/// Open the compositor's shortcut editor; falls back to KDE's shortcuts KCM.
pub async fn configure_shortcuts() {
    if let Some(sc) = SHORTCUTS.get() {
        if sc.gs.configure_shortcuts(&sc.session, None, Default::default()).await.is_ok() {
            return;
        }
    }
    // Hyprland has no shortcut editor: bindings live in hyprland.conf (see describe_shortcuts).
    if !super::hyprland::detected() {
        let _ = std::process::Command::new("systemsettings").arg("kcm_keys").spawn();
    }
}

fn token_path() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("lootingglass/remote-desktop.token")
}

async fn open_keyboard() -> Result<Keyboard> {
    let rd = RemoteDesktop::new().await?;
    let session = rd.create_session(Default::default()).await?;
    let token = std::fs::read_to_string(token_path()).ok();
    rd.select_devices(
        &session,
        SelectDevicesOptions::default()
            .set_devices(DeviceType::KEYBOARD)
            .set_persist_mode(PersistMode::ExplicitlyRevoked)
            .set_restore_token(token.as_deref()),
    )
    .await?;
    let started = rd.start(&session, None, Default::default()).await?.response()?;
    if let Some(t) = started.restore_token() {
        let p = token_path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, t);
    }
    Ok(Keyboard { rd, session })
}

async fn press(kb: &Keyboard, keys: &[i32]) -> Result<()> {
    // Games often sample key state once per frame, so space events out by more than a frame.
    const GAP: Duration = Duration::from_millis(25);
    for &k in keys {
        kb.rd.notify_keyboard_keycode(&kb.session, k, KeyState::Pressed, Default::default()).await?;
        tokio::time::sleep(GAP).await;
    }
    for &k in keys.iter().rev() {
        kb.rd.notify_keyboard_keycode(&kb.session, k, KeyState::Released, Default::default()).await?;
        tokio::time::sleep(GAP).await;
    }
    Ok(())
}

/// Send Ctrl+C to the focused window. First use shows KDE's "allow remote control" dialog;
/// afterwards the grant is restored silently from the saved token.
pub async fn send_copy() -> Result<()> {
    let mut guard = KEYBOARD.lock().await;
    for attempt in 0..2 {
        if guard.is_none() {
            *guard = Some(open_keyboard().await.context("RemoteDesktop portal refused keyboard access")?);
        }
        match press(guard.as_ref().unwrap(), &[KEY_LEFTCTRL, KEY_C]).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt == 0 => {
                log::warn!("keyboard session lost, reconnecting: {e}");
                *guard = None;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

fn read_clipboard() -> Option<String> {
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};
    let (mut pipe, _) = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text)
        .map_err(|e| log::debug!("clipboard read: {e}"))
        .ok()?;
    let mut s = String::new();
    pipe.read_to_string(&mut s).ok()?;
    Some(s)
}

/// Open the keyboard session up front: the first key event on a fresh session can get lost,
/// and opening it on the first price check delays that check.
pub async fn warm_up_keyboard() {
    let mut guard = KEYBOARD.lock().await;
    if guard.is_none() {
        match open_keyboard().await {
            Ok(kb) => *guard = Some(kb),
            Err(e) => log::warn!("keyboard portal: {e:#}"),
        }
    }
}

/// Make the game copy the hovered item and return the clipboard text.
pub async fn copy_item() -> Result<String> {
    // Clear first so a missed keystroke can't silently return the previous item.
    tokio::task::spawn_blocking(|| {
        use wl_clipboard_rs::copy::{ClipboardType, Seat, clear};
        if let Err(e) = clear(ClipboardType::Regular, Seat::All) {
            log::warn!("clipboard clear: {e}");
        }
    })
    .await?;
    // The overlay may have held keyboard focus; make sure Ctrl+C reaches the game.
    super::focus_game().await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    if super::hyprland::detected() {
        super::hyprland::send_copy().await?;
    } else {
        send_copy().await?;
    }
    // Wine/XWayland can take a while to publish the selection; poll for up to 1.5s.
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(now) = tokio::task::spawn_blocking(read_clipboard).await? {
            if !now.trim().is_empty() {
                return Ok(now);
            }
        }
    }
    anyhow::bail!("The game didn't copy anything — is the cursor over an item and the game focused?")
}
