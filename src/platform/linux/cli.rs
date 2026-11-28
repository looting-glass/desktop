//! Running the compositors' command-line tools (`hyprctl`, `swaymsg`, `umbriel`), which the
//! focus and key-sending providers talk to the compositor through.

use anyhow::{Context, Result, bail};

/// Run `program` and return what it printed. When it exits with an error, fail with what it
/// said about it.
pub async fn run(program: &str, args: &[&str]) -> Result<String> {
    let out = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("running {program}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        bail!("{program} {}: {}", out.status, error_text(&String::from_utf8_lossy(&out.stderr), &stdout));
    }
    Ok(stdout.into_owned())
}

/// What a failed tool said went wrong: its error output, or its normal output when it reports
/// errors there (`swaymsg` prints `[{"success": false, "error": "No matching node."}]`).
fn error_text<'a>(stderr: &'a str, stdout: &'a str) -> &'a str {
    match stderr.trim() {
        "" => stdout.trim(),
        stderr => stderr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_text_prefers_the_error_output() {
        assert_eq!(error_text("Error: no such window\n", "[]"), "Error: no such window");
    }

    #[test]
    fn error_text_falls_back_to_the_normal_output() {
        assert_eq!(error_text("  \n", "No matching node.\n"), "No matching node.");
        assert_eq!(error_text("", ""), "");
    }
}
