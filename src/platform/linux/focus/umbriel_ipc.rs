//! Focus the game through Umbriel's IPC: find its window with `umbriel windows --json`, then
//! `umbriel msg window-focus <id>`. EXPERIMENTAL, not yet tested on a real Umbriel session.

use crate::platform::linux::{GAME_TITLE, cli};
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

pub async fn focus_game() {
    if let Err(e) = run().await {
        log::warn!("could not refocus the game via Umbriel: {e:#}");
    }
}

async fn run() -> Result<()> {
    let listing = cli::run("umbriel", &["windows", "--json"]).await?;
    let Some(id) = game_window_id(&listing)? else {
        bail!("the game window isn't open");
    };
    cli::run("umbriel", &["msg", "window-focus", &id]).await?;
    Ok(())
}

/// `umbriel windows --json`. Umbriel's docs name the fields but don't show the whole reply, which
/// is either the list itself or, as on its socket, wrapped in `{"ok": …}`.
#[derive(Deserialize)]
#[serde(untagged)]
enum Listing {
    Windows(Vec<Window>),
    Reply { ok: Vec<Window> },
}

#[derive(Deserialize)]
struct Window {
    id: serde_json::Value,
    #[serde(default)]
    title: String,
}

/// The id of the game's window in Umbriel's window listing, if it's open.
fn game_window_id(listing: &str) -> Result<Option<String>> {
    static GAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(GAME_TITLE).unwrap());
    let (Listing::Windows(windows) | Listing::Reply { ok: windows }) =
        serde_json::from_str(listing).with_context(|| format!("unexpected window listing {listing:?}"))?;
    Ok(windows.into_iter().find(|w| GAME.is_match(&w.title)).map(|w| match w.id {
        serde_json::Value::String(id) => id,
        id => id.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#"[
        { "id": "7", "app_id": "firefox", "title": "Trade - Path of Exile — Mozilla Firefox", "focused": false },
        { "id": "12", "app_id": "steam_app_2694490", "title": "Path of Exile 2", "focused": true }
    ]"#;

    #[test]
    fn finds_the_game_not_the_trade_site() {
        assert_eq!(game_window_id(LISTING).unwrap().as_deref(), Some("12"));
    }

    #[test]
    fn reads_the_socket_form_and_numeric_ids() {
        let listing = r#"{ "ok": [ { "id": 3, "title": "Path of Exile" } ] }"#;
        assert_eq!(game_window_id(listing).unwrap().as_deref(), Some("3"));
    }

    #[test]
    fn no_game_window() {
        assert_eq!(game_window_id("[]").unwrap(), None);
        assert_eq!(game_window_id(r#"[ { "id": "1", "title": "Path of Exile Wiki" } ]"#).unwrap(), None);
    }

    #[test]
    fn malformed_listing() {
        for listing in ["", "not json", r#"{ "err": "no such command" }"#] {
            assert!(game_window_id(listing).is_err(), "{listing:?}");
        }
    }
}
