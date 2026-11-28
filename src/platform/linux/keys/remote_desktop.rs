//! Ctrl+C through the RemoteDesktop portal's keyboard (keyboard only, no screen sharing).

use super::{COPY_CHORD, KEY_GAP};
use anyhow::{Context, Result};
use ashpd::desktop::{
    PersistMode, Session,
    remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions},
};
use std::path::PathBuf;
use tokio::sync::Mutex;

struct Keyboard {
    rd: RemoteDesktop,
    session: Session<RemoteDesktop>,
}
static KEYBOARD: Mutex<Option<Keyboard>> = Mutex::const_new(None);

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

async fn press_copy(kb: &Keyboard) -> Result<()> {
    for (key, pressed) in COPY_CHORD {
        let state = if pressed { KeyState::Pressed } else { KeyState::Released };
        kb.rd.notify_keyboard_keycode(&kb.session, key as i32, state, Default::default()).await?;
        tokio::time::sleep(KEY_GAP).await;
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
        match press_copy(guard.as_ref().unwrap()).await {
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

/// Open the keyboard session up front: the first key event on a fresh session can get lost,
/// and opening it on the first price check delays that check.
pub async fn warm_up() {
    let mut guard = KEYBOARD.lock().await;
    if guard.is_none() {
        match open_keyboard().await {
            Ok(kb) => *guard = Some(kb),
            Err(e) => log::warn!("keyboard portal: {e:#}"),
        }
    }
}
