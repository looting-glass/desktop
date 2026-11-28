//! Desktop notifications (org.freedesktop.Notifications): what the tray's "Check for updates…"
//! answers with, and the "updated to …" notice after a self-update. Clicking one can send an app
//! event (e.g. open the About page).

use crate::Event;
use anyhow::Result;
use ashpd::zbus::{Connection, Proxy, Variant};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::Duration;

const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

/// Show a notification; `on_click` is sent when the user clicks it. False when no notification
/// service answers (the caller then shows the information some other way).
pub async fn show(summary: &str, body: &str, on_click: Option<Event>) -> bool {
    match try_show(summary, body, on_click).await {
        Ok(()) => true,
        Err(e) => {
            log::warn!("notification: {e:#}");
            false
        }
    }
}

async fn try_show(summary: &str, body: &str, on_click: Option<Event>) -> Result<()> {
    let conn = Connection::session().await?;
    let proxy = Proxy::new(&conn, SERVICE, PATH, SERVICE).await?;
    // Listen before showing it, so a quick click isn't missed.
    let clicks = match on_click {
        Some(_) => Some(proxy.receive_signal("ActionInvoked").await?),
        None => None,
    };
    let actions: Vec<&str> = if on_click.is_some() { vec!["default", "Open"] } else { vec![] };
    let mut hints: HashMap<&str, Variant> = HashMap::new();
    // Groups it under Looting Glass in the desktop's notification settings and history.
    hints.insert("desktop-entry", Variant::from(super::APP_ID));
    let id: u32 = proxy
        .call("Notify", &("Looting Glass", 0u32, super::APP_ID, summary, body, actions, hints, -1i32))
        .await?;
    if let (Some(mut clicks), Some(ev)) = (clicks, on_click) {
        tokio::spawn(async move {
            let wait = async {
                while let Some(msg) = clicks.next().await {
                    if let Ok((nid, _action)) = msg.body().deserialize::<(u32, String)>()
                        && nid == id
                    {
                        crate::send_event(ev);
                        return;
                    }
                }
            };
            // Stop listening once it's long gone from the screen and the history.
            let _ = tokio::time::timeout(Duration::from_secs(30 * 60), wait).await;
        });
    }
    Ok(())
}
