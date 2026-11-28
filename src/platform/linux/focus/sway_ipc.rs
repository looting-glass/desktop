//! Focus the game through Sway's IPC, with a `swaymsg` command. EXPERIMENTAL, not yet tested on
//! a real Sway session.

use crate::platform::linux::GAME_TITLE;
use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub async fn focus_game() {
    if let Err(e) = run().await {
        log::warn!("could not refocus the game via Sway: {e:#}");
    }
}

async fn run() -> Result<()> {
    let out = tokio::process::Command::new("swaymsg")
        .arg(format!(r#"[title="{GAME_TITLE}"] focus"#))
        .output()
        .await
        .context("running swaymsg")?;
    command_outcome(&String::from_utf8_lossy(&out.stdout))
}

/// One command's result in Sway's reply.
#[derive(Deserialize)]
struct Outcome {
    success: bool,
    #[serde(default)]
    error: String,
}

/// Whether Sway ran the command, from its reply: `[{"success": true}]`, or
/// `[{"success": false, "error": "No matching node."}]` when the game isn't open.
fn command_outcome(reply: &str) -> Result<()> {
    let outcomes: Vec<Outcome> = serde_json::from_str(reply).with_context(|| format!("unexpected reply {reply:?}"))?;
    if outcomes.is_empty() {
        bail!("empty reply");
    }
    match outcomes.into_iter().find(|o| !o.success) {
        Some(failed) => bail!("{}", failed.error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success() {
        assert!(command_outcome(r#"[ { "success": true } ]"#).is_ok());
    }

    #[test]
    fn no_game_window() {
        let e = command_outcome(r#"[ { "success": false, "parse_error": false, "error": "No matching node." } ]"#);
        assert_eq!(e.unwrap_err().to_string(), "No matching node.");
    }

    #[test]
    fn empty_or_malformed_reply() {
        for reply in ["", "[]", "{}", "not json"] {
            assert!(command_outcome(reply).is_err(), "{reply:?}");
        }
    }
}
