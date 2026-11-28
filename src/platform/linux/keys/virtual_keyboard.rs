//! Ctrl+C through a `zwp_virtual_keyboard_v1` keyboard, which Sway, Umbriel and other
//! wlroots-style compositors offer to any client. EXPERIMENTAL, not yet tested with the game.

use super::{COPY_CHORD, KEY_GAP, KEY_LEFTCTRL};
use anyhow::{Context, Result};
use rustix::fs::{MemfdFlags, memfd_create};
use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::time::Instant;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_keyboard::KeymapFormat, wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

/// A virtual keyboard brings its own keymap. This one has just the two keys we press, mapped to
/// Ctrl and C whatever the user's layout, so the game gets Ctrl+C on Dvorak too. Keycodes are
/// evdev codes plus 8: `<LCTL>` is `KEY_LEFTCTRL`, `<AB03>` is `KEY_C`.
const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "lootingglass" {
        minimum = 8;
        maximum = 255;
        <LCTL> = 37;
        <AB03> = 54;
    };
    xkb_types "lootingglass" { include "complete" };
    xkb_compatibility "lootingglass" { include "complete" };
    xkb_symbols "lootingglass" {
        key <LCTL> { [ Control_L ] };
        key <AB03> { [ c, C ] };
        modifier_map Control { <LCTL> };
    };
};
"#;

/// Control is the third of XKB's fixed real modifiers (Shift, Lock, Control, …).
const CONTROL_MASK: u32 = 1 << 2;

/// Nothing to receive: we only send requests.
struct NoEvents;

impl Dispatch<WlRegistry, GlobalListContents> for NoEvents {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(NoEvents: ignore WlSeat);
delegate_noop!(NoEvents: ZwpVirtualKeyboardManagerV1);
delegate_noop!(NoEvents: ZwpVirtualKeyboardV1);

/// Send Ctrl+C to the focused window. Blocking.
pub fn send_copy() -> Result<()> {
    let conn = Connection::connect_to_env().context("No Wayland connection to send Ctrl+C through")?;
    let (globals, mut queue) = registry_queue_init::<NoEvents>(&conn)?;
    let qh = queue.handle();
    let seat: WlSeat = globals.bind(&qh, 1..=1, ())?;
    let manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
    let keymap = keymap_file()?;
    keyboard.keymap(KeymapFormat::XkbV1.into(), keymap.as_fd(), KEYMAP.len() as u32 + 1);

    let start = Instant::now();
    for (key, pressed) in COPY_CHORD {
        let time = u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX);
        keyboard.key(time, key, u32::from(pressed));
        // Compositors that don't track modifiers from a virtual keyboard's keys (Smithay's, for
        // one) need them spelled out, so the game sees Ctrl held while C goes down.
        if key == KEY_LEFTCTRL {
            keyboard.modifiers(if pressed { CONTROL_MASK } else { 0 }, 0, 0, 0);
        }
        conn.flush()?;
        std::thread::sleep(KEY_GAP);
    }
    keyboard.destroy();
    // A refused keyboard shows up as a protocol error here rather than as silence.
    queue.roundtrip(&mut NoEvents).context("The compositor refused the virtual keyboard")?;
    Ok(())
}

/// The keymap in an anonymous file, NUL-terminated as the protocol expects.
fn keymap_file() -> Result<File> {
    let mut file = File::from(memfd_create("lootingglass-keymap", MemfdFlags::CLOEXEC)?);
    file.write_all(KEYMAP.as_bytes())?;
    file.write_all(b"\0")?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::super::KEY_C;
    use super::*;

    #[test]
    fn keymap_maps_the_chord_keys_to_ctrl_and_c() {
        assert!(KEYMAP.contains(&format!("<LCTL> = {};", KEY_LEFTCTRL + 8)));
        assert!(KEYMAP.contains(&format!("<AB03> = {};", KEY_C + 8)));
    }
}
