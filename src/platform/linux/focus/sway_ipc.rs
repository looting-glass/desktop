//! Focus the game through Sway's IPC, with a `swaymsg` command. EXPERIMENTAL, not yet tested on
//! a real Sway session.

use crate::platform::linux::{GAME_TITLE, cli};

pub async fn focus_game() {
    // swaymsg exits with an error when the command fails, with "No matching node." when the game
    // isn't open.
    if let Err(e) = cli::run("swaymsg", &[&format!(r#"[title="{GAME_TITLE}"] focus"#)]).await {
        log::warn!("could not refocus the game via Sway: {e:#}");
    }
}
