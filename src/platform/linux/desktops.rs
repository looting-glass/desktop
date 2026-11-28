//! The "Supported desktops" table in `docs/desktops.md`: what `select` picks for each job on the
//! desktops we know, with a short hand-kept status for each. The website shows `docs/` as plain
//! Markdown without running anything, so the table is committed, and `desktops_table_is_current`
//! fails when it no longer matches.

use super::facts::Facts;
use super::fixtures::*;
use super::select::{Job, Missing, select};

/// How far a desktop is supported. This is about testing, not protocols: a desktop stays
/// experimental until someone has run it with the game.
enum Status {
    Tested,
    Experimental,
    /// Why, in a sentence for the user.
    NotSupported(&'static str),
}

impl Status {
    fn markdown(&self) -> String {
        match self {
            Self::Tested => "Supported and tested.".into(),
            Self::Experimental => "**Experimental**: not yet run with the game.".into(),
            Self::NotSupported(why) => format!("**Not supported**: {why}"),
        }
    }
}

/// A row of the docs table.
struct Known {
    name: &'static str,
    facts: fn() -> Facts,
    /// Its section of `docs/shortcuts.md`, which says how to bind the keys there.
    shortcuts_doc: Option<&'static str>,
    status: Status,
}

impl Known {
    /// The table cell for `job`: its provider's name, linked to how to bind the keys there.
    fn cell(&self, job: Job, provider: Result<&str, Missing>) -> String {
        match (job, provider, self.shortcuts_doc) {
            (_, Err(_), _) => "**missing**".into(),
            (Job::Shortcuts, Ok(name), Some(section)) => format!("[{name}](shortcuts.md#{section})"),
            (_, Ok(name), _) => name.into(),
        }
    }
}

/// In the order the docs list them.
const KNOWN: &[Known] = &[
    Known { name: "KDE Plasma 6", facts: kde, shortcuts_doc: Some("kde-plasma"), status: Status::Tested },
    Known { name: "Hyprland", facts: hyprland, shortcuts_doc: Some("hyprland-experimental"), status: Status::Experimental },
    Known { name: "Sway", facts: sway, shortcuts_doc: Some("sway-experimental"), status: Status::Experimental },
    Known { name: "Umbriel", facts: umbriel, shortcuts_doc: Some("umbriel-experimental"), status: Status::Experimental },
    Known { name: "river, Wayfire", facts: wlroots, shortcuts_doc: Some("other-desktops"), status: Status::Experimental },
    Known {
        name: "GNOME",
        facts: gnome_48,
        shortcuts_doc: None,
        status: Status::NotSupported(
            "GNOME doesn't let apps show prices over the game, read what the game copied or switch you back to it. [Why GNOME isn't supported](#why-gnome-isnt-supported) explains.",
        ),
    },
];

/// The "Supported desktops" table: what Looting Glass picks for each job on each desktop.
fn table() -> String {
    let jobs = Job::ALL.map(Job::name).join(" | ");
    let mut table = format!("| Desktop | {jobs} | Status |\n|---|{}---|\n", "---|".repeat(Job::ALL.len()));
    for known in KNOWN {
        let selection = select(&(known.facts)());
        let cells = Job::ALL.map(|job| known.cell(job, selection.provider(job))).join(" | ");
        table += &format!("| {} | {cells} | {} |\n", known.name, known.status.markdown());
    }
    table
}

const TABLE_START: &str = "<!-- Generated from src/platform/linux/desktops.rs by `LOOTINGGLASS_UPDATE_DOCS=1 cargo test desktops_table` -->\n";
const TABLE_END: &str = "<!-- End of the generated table -->";

/// `doc` with the text between the table markers replaced by `table`, or None without markers.
fn with_table(doc: &str, table: &str) -> Option<String> {
    let start = doc.find(TABLE_START)? + TABLE_START.len();
    let end = start + doc[start..].find(TABLE_END)?;
    Some(format!("{}{table}{}", &doc[..start], &doc[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktops_table_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/desktops.md");
        let doc = std::fs::read_to_string(&path).expect("read docs/desktops.md");
        let current = with_table(&doc, &table()).expect("docs/desktops.md lost its generated-table markers");
        if current == doc {
            return;
        }
        if std::env::var_os("LOOTINGGLASS_UPDATE_DOCS").is_some() {
            std::fs::write(&path, current).unwrap();
        } else {
            panic!("The desktops table in docs/desktops.md is out of date. Run: LOOTINGGLASS_UPDATE_DOCS=1 cargo test desktops_table");
        }
    }

    #[test]
    fn missing_jobs_stand_out_and_shortcuts_link_to_their_binding_help() {
        let sway = &KNOWN[2];
        assert_eq!(sway.cell(Job::Shortcuts, Ok("command")), "[command](shortcuts.md#sway-experimental)");
        assert_eq!(sway.cell(Job::Focus, Ok("Sway IPC")), "Sway IPC");
        assert_eq!(sway.cell(Job::Clipboard, Err(Missing::CLIPBOARD)), "**missing**");
    }

    #[test]
    fn the_table_goes_between_the_markers() {
        let doc = format!("Intro\n{TABLE_START}old\n{TABLE_END}\nOutro\n");
        assert_eq!(with_table(&doc, "new\n"), Some(format!("Intro\n{TABLE_START}new\n{TABLE_END}\nOutro\n")));
        assert_eq!(with_table("No markers", "new\n"), None);
    }
}
