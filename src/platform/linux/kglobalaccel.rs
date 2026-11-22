//! KDE's global shortcut service (kglobalaccel, on D-Bus): read and change the keys of the
//! shortcuts Looting Glass registered through the GlobalShortcuts portal, so they can be edited
//! in our own settings window. The same calls System Settings uses; changes show up there too.
//! Keys are Qt key combinations (`Qt::CTRL | Qt::Key_D` = 0x04000044), as QML key events give them.

use anyhow::{Context, Result, bail};
use ashpd::zbus::Connection;

const SERVICE: &str = "org.kde.kglobalaccel";
const IFACE: &str = "org.kde.KGlobalAccel";
const COMPONENT: &str = super::portal::APP_ID;
const COMPONENT_NAME: &str = "Looting Glass";

/// `(actionUnique, actionFriendly, componentUnique, componentFriendly, contextUnique,
/// contextFriendly, keys, defaultKeys)`, kglobalaccel's KGlobalShortcutInfo on D-Bus.
type Info = (String, String, String, String, String, String, Vec<i32>, Vec<i32>);

async fn conn() -> Result<Connection> {
    Ok(Connection::session().await?)
}

fn action_id(id: &str, description: &str) -> Vec<String> {
    vec![COMPONENT.into(), id.into(), COMPONENT_NAME.into(), description.into()]
}

/// Our shortcuts as (id, description, keys); fails when kglobalaccel isn't running (not KDE)
/// or doesn't know our component yet.
pub async fn list() -> Result<Vec<(String, String, Vec<i32>)>> {
    let c = conn().await?;
    let reply = c
        .call_method(Some(SERVICE), "/kglobalaccel", Some(IFACE), "getComponent", &(COMPONENT,))
        .await
        .context("kglobalaccel getComponent")?;
    let path: ashpd::zbus::OwnedObjectPath = reply.body().deserialize()?;
    let reply = c
        .call_method(Some(SERVICE), path.as_str(), Some("org.kde.kglobalaccel.Component"), "allShortcutInfos", &())
        .await
        .context("kglobalaccel allShortcutInfos")?;
    let infos: Vec<Info> = reply.body().deserialize()?;
    Ok(infos.into_iter().map(|i| (i.0, i.1, i.6.into_iter().filter(|k| *k != 0).collect())).collect())
}

/// Another shortcut using `key`, if any: (who, e.g. "Session Management: Lock Session", its info).
async fn user_of(c: &Connection, id: &str, key: i32) -> Result<Option<(String, Info)>> {
    let reply = c
        .call_method(Some(SERVICE), "/kglobalaccel", Some(IFACE), "globalShortcutsByKey", &((vec![key, 0, 0, 0],), (0i32,)))
        .await
        .context("kglobalaccel globalShortcutsByKey")?;
    let users: Vec<Info> = reply.body().deserialize()?;
    Ok(users.into_iter().find(|u| !(u.2 == COMPONENT && u.0 == id)).map(|u| (format!("{}: {}", u.3, u.1), u)))
}

/// Who already uses `key` (e.g. "Session Management: Lock Session"), or None if it's free for `id`.
pub async fn conflict(id: &str, key: i32) -> Result<Option<String>> {
    if key == 0 {
        return Ok(None);
    }
    Ok(user_of(&conn().await?, id, key).await?.map(|(who, _)| who))
}

/// Bind `key` (0 = none) to shortcut `id`. A key another shortcut uses is taken from it when
/// `take_over` is set (like System Settings' "Reassign"), otherwise refused.
pub async fn set(id: &str, description: &str, key: i32, take_over: bool) -> Result<()> {
    let c = conn().await?;
    if key != 0
        && let Some((who, u)) = user_of(&c, id, key).await?
    {
        if !take_over {
            bail!("{} is already used by {who}", key_text(key));
        }
        let rest: Vec<(Vec<i32>,)> = u.6.iter().filter(|k| **k != 0 && **k != key).map(|k| (vec![*k, 0, 0, 0],)).collect();
        let other = vec![u.2.clone(), u.0.clone(), u.3.clone(), u.1.clone()];
        c.call_method(Some(SERVICE), "/kglobalaccel", Some(IFACE), "setForeignShortcutKeys", &(other, rest))
            .await
            .context("kglobalaccel: taking the key from its old shortcut")?;
        log::info!("took {} from {who}", key_text(key));
    }
    let keys: Vec<(Vec<i32>,)> = if key == 0 { vec![] } else { vec![(vec![key, 0, 0, 0],)] };
    c.call_method(Some(SERVICE), "/kglobalaccel", Some(IFACE), "setForeignShortcutKeys", &(action_id(id, description), keys))
        .await
        .context("kglobalaccel setForeignShortcutKeys")?;
    Ok(())
}

/// Pause all global shortcuts while the user presses a new key combination in the settings
/// window, so pressing it there doesn't also trigger whatever it's bound to.
pub async fn block(on: bool) -> Result<()> {
    conn()
        .await?
        .call_method(Some(SERVICE), "/kglobalaccel", Some(IFACE), "blockGlobalShortcuts", &(on,))
        .await?;
    Ok(())
}

const SHIFT: i32 = 0x0200_0000;
const CTRL: i32 = 0x0400_0000;
const ALT: i32 = 0x0800_0000;
const META: i32 = 0x1000_0000;
const KEYPAD: i32 = 0x2000_0000;

/// "Meta+Ctrl+Alt+Shift+F7"-style text for a Qt key combination, like KDE shows it.
pub fn key_text(combo: i32) -> String {
    let mut parts: Vec<String> = [(META, "Meta"), (CTRL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift")]
        .iter()
        .filter(|(m, _)| combo & m != 0)
        .map(|(_, n)| n.to_string())
        .collect();
    let key = combo & !(SHIFT | CTRL | ALT | META | KEYPAD);
    let name = match key {
        0x20 => "Space".to_string(),
        0x21..=0x7e => (key as u8 as char).to_ascii_uppercase().to_string(),
        0x0100_0030..=0x0100_0052 => format!("F{}", key - 0x0100_0030 + 1),
        _ => match key {
            0x0100_0000 => "Esc",
            0x0100_0001 => "Tab",
            0x0100_0003 => "Backspace",
            0x0100_0004 => "Return",
            0x0100_0005 => "Enter",
            0x0100_0006 => "Ins",
            0x0100_0007 => "Del",
            0x0100_0008 => "Pause",
            0x0100_0009 => "Print",
            0x0100_0010 => "Home",
            0x0100_0011 => "End",
            0x0100_0012 => "Left",
            0x0100_0013 => "Up",
            0x0100_0014 => "Right",
            0x0100_0015 => "Down",
            0x0100_0016 => "PgUp",
            0x0100_0017 => "PgDown",
            _ => return format!("{}{:#x}", parts.iter().map(|p| format!("{p}+")).collect::<String>(), key),
        }
        .to_string(),
    };
    parts.push(name);
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_combinations_read_like_kde() {
        assert_eq!(key_text(67108932), "Ctrl+D", "price check's default, as kglobalaccel returns it");
        assert_eq!(key_text(CTRL | SHIFT | 0x0100_0036), "Ctrl+Shift+F7");
        assert_eq!(key_text(META | ALT | 0x31), "Meta+Alt+1");
        assert_eq!(key_text(0x0100_0030), "F1");
        assert_eq!(key_text(CTRL | 0x20), "Ctrl+Space");
        assert_eq!(key_text(KEYPAD | 0x35), "5");
    }
}
