//! Windows backend — EXPERIMENTAL, not yet tested with the game.
//!
//! - hotkeys:   RegisterHotKey on a dedicated thread; bindings live in config.toml
//!              (`[shortcuts]`) and are edited in Looting Glass's own settings
//! - copy_item: SendInput Ctrl+C to the game, then read the Win32 clipboard
//! - focus:     SetForegroundWindow on the game window (class `POEWindowClass`)
//! - tray:      QSystemTrayIcon in cpp/main.cpp, calling back into `lootingglass_tray_*` below
//! - overlay:   frameless always-on-top Qt::Tool window (cpp/main.cpp); PoE must run in
//!              windowed fullscreen, exclusive fullscreen can't be overlaid

use super::hotkey;
use crate::Event;
use crate::config::Game;
use anyhow::{Result, bail};
use std::sync::OnceLock;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HANDLE, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
};
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, MOD_NOREPEAT, RegisterHotKey,
    SendInput, UnregisterHotKey, VK_C, VK_CONTROL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    SetForegroundWindow, WM_APP, WM_HOTKEY,
};

/// Window class of both PoE 1 and PoE 2.
const GAME_CLASS: &str = "POEWindowClass";
/// Posted to the hotkey thread to re-register hotkeys after the config changed.
const WM_RELOAD: u32 = WM_APP + 1;

static HOTKEY_THREAD: OnceLock<u32> = OnceLock::new();

pub async fn init() {}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ── Hotkeys ──────────────────────────────────────────────────────────────────

/// Hotkey id (index) → (config value, event).
fn bindings() -> Vec<(String, Event)> {
    let s = crate::config().shortcuts;
    vec![
        (s.price_check, Event::PriceCheck),
        (s.toggle_game, Event::ToggleGame),
        (s.manual_check, Event::OpenManual),
        (s.settings, Event::OpenSettings),
        (s.hide_overlay, Event::HideOverlay),
        (s.item_info, Event::OpenItemInfo),
    ]
}

/// (Re-)register all configured hotkeys for the current thread; returns the events by id.
fn register_all(previous: usize) -> Vec<Event> {
    for id in 0..previous {
        unsafe { UnregisterHotKey(std::ptr::null_mut(), id as i32) };
    }
    let mut events = Vec::new();
    for (id, (key, ev)) in bindings().into_iter().enumerate() {
        events.push(ev);
        if key.trim().is_empty() {
            continue;
        }
        let Some((mods, vk)) = hotkey::parse(&key) else {
            log::warn!("invalid hotkey {key:?}");
            continue;
        };
        if unsafe { RegisterHotKey(std::ptr::null_mut(), id as i32, mods | MOD_NOREPEAT, vk) } == 0 {
            log::warn!("hotkey {key} is already taken by another application");
        }
    }
    events
}

pub async fn run_shortcuts(tx: async_channel::Sender<Event>) -> Result<()> {
    std::thread::Builder::new().name("hotkeys".into()).spawn(move || unsafe {
        let mut msg: MSG = std::mem::zeroed();
        // Create this thread's message queue before anyone posts WM_RELOAD to it.
        PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        let _ = HOTKEY_THREAD.set(GetCurrentThreadId());
        let mut events = register_all(0);
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            match msg.message {
                WM_HOTKEY => {
                    if let Some(ev) = events.get(msg.wParam) {
                        let _ = tx.send_blocking(ev.clone());
                    }
                }
                WM_RELOAD => events = register_all(events.len()),
                _ => {}
            }
        }
    })?;
    Ok(())
}

pub fn reload_shortcuts() {
    if let Some(&tid) = HOTKEY_THREAD.get() {
        unsafe { PostThreadMessageW(tid, WM_RELOAD, 0, 0) };
    }
}

/// Windows registers its own hotkeys, so there's no `lootingglass trigger` to hand them over.
pub fn trigger(_id: &str) -> Result<()> {
    bail!("On Windows, set hotkeys in Looting Glass's Settings → Shortcuts instead.")
}

/// Windows hotkeys are edited as text in the settings window (`[shortcuts]` in the config).
pub async fn shortcut_list() -> super::ShortcutList {
    super::ShortcutList { mode: "app", ..Default::default() }
}

pub async fn set_shortcut(_id: &str, _key: i32) -> anyhow::Result<()> {
    anyhow::bail!("Windows hotkeys are set in the settings fields")
}

pub async fn block_shortcuts(_on: bool) {}

/// No desktop notifications on Windows yet: callers show the information in the app instead.
pub async fn notify(_summary: &str, _body: &str, _on_click: Option<crate::Event>) -> bool {
    false
}

pub async fn check_shortcut(id: &str, key: i32) -> super::ShortcutCheck {
    super::ShortcutCheck { id: id.into(), key, ..Default::default() }
}

/// Hotkeys are edited directly in the settings window on Windows.
pub async fn configure_shortcuts() {}

// ── Copying the item ─────────────────────────────────────────────────────────

fn key(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_copy() -> Result<()> {
    let inputs = [key(VK_CONTROL, false), key(VK_C, false), key(VK_C, true), key(VK_CONTROL, true)];
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        bail!("Windows blocked the simulated Ctrl+C (is the game running as administrator?)");
    }
    Ok(())
}

/// Open the clipboard, retrying briefly: another app (or the game) may hold it.
fn with_clipboard<T>(f: impl FnOnce() -> T) -> Option<T> {
    for _ in 0..10 {
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            let r = f();
            unsafe { CloseClipboard() };
            return Some(r);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

fn clear_clipboard() {
    with_clipboard(|| unsafe { EmptyClipboard() });
}

fn read_clipboard() -> Option<String> {
    with_clipboard(|| unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT as u32) == 0 {
            return None;
        }
        let h: HANDLE = GetClipboardData(CF_UNICODETEXT as u32);
        if h.is_null() {
            return None;
        }
        let p = GlobalLock(h) as *const u16;
        if p.is_null() {
            return None;
        }
        let len = (0..).take_while(|&i| *p.add(i) != 0).count();
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        GlobalUnlock(h);
        Some(s)
    })
    .flatten()
}

/// Make the game copy the hovered item and return the clipboard text.
pub async fn copy_item() -> Result<String> {
    // Clear first so a missed keystroke can't silently return the previous item.
    tokio::task::spawn_blocking(clear_clipboard).await?;
    focus_game().await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    tokio::task::spawn_blocking(send_copy).await??;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(now) = tokio::task::spawn_blocking(read_clipboard).await? {
            if !now.trim().is_empty() {
                return Ok(now.replace("\r\n", "\n"));
            }
        }
    }
    bail!("The game didn't copy anything — is the cursor over an item and the game focused?")
}

/// Return keyboard focus to the game after the overlay closes.
pub async fn focus_game() {
    let class = wide(GAME_CLASS);
    let hwnd: HWND = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
    if hwnd.is_null() {
        log::debug!("game window not found");
    } else if unsafe { SetForegroundWindow(hwnd) } == 0 {
        log::debug!("SetForegroundWindow refused");
    }
}

// ── Tray (QSystemTrayIcon in cpp/main.cpp) ───────────────────────────────────

pub async fn spawn_tray(_game: Game) {}

/// The tray menu reads the current game when it opens, nothing to push.
pub async fn set_tray_game(_game: Game) {}

/// Newer version for the tray menu, read by `lootingglass_tray_update` when the menu opens.
static TRAY_UPDATE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub async fn set_tray_update(version: Option<String>) {
    *TRAY_UPDATE.lock().unwrap() = version;
}

/// Copies the newer version (NUL-terminated UTF-8) into `buf`; returns 0 when there is none.
#[unsafe(no_mangle)]
extern "C" fn lootingglass_tray_update(buf: *mut u8, len: usize) -> i32 {
    let v = TRAY_UPDATE.lock().unwrap();
    let Some(v) = v.as_deref() else { return 0 };
    if buf.is_null() || v.len() + 1 > len {
        return 0;
    }
    // SAFETY: the caller passes a writable buffer of `len` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(v.as_ptr(), buf, v.len());
        *buf.add(v.len()) = 0;
    }
    1
}

#[unsafe(no_mangle)]
extern "C" fn lootingglass_tray_action(action: i32) {
    let ev = match action {
        0 => Event::OpenManual,
        1 => Event::OpenSettings,
        2 => Event::Quit,
        3 => Event::OpenAbout,
        4 => Event::CheckUpdates,
        10 => Event::SetGame(Game::Poe1),
        11 => Event::SetGame(Game::Poe2),
        _ => return,
    };
    crate::send_event(ev);
}

#[unsafe(no_mangle)]
extern "C" fn lootingglass_tray_game() -> i32 {
    match crate::config().game {
        Game::Poe1 => 1,
        Game::Poe2 => 2,
    }
}
