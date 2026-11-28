//! The GlobalShortcuts portal: hotkeys that work while the game has focus, bound in the
//! desktop's own shortcut settings.

use super::SHORTCUT_DEFS;
use crate::Event;
use crate::platform::BindHelp;
use anyhow::{Context, Result};
use ashpd::desktop::{
    Session,
    global_shortcuts::{GlobalShortcuts, NewShortcut},
};
use futures_util::StreamExt;
use tokio::sync::OnceCell;

struct Shortcuts {
    gs: GlobalShortcuts,
    session: Session<GlobalShortcuts>,
}
static SHORTCUTS: OnceCell<Shortcuts> = OnceCell::const_new();

pub async fn run(tx: async_channel::Sender<Event>) -> Result<()> {
    let sc = SHORTCUTS
        .get_or_try_init(|| async {
            let gs = GlobalShortcuts::new().await?;
            let session = gs.create_session(Default::default()).await?;
            let shortcuts: Vec<NewShortcut> = SHORTCUT_DEFS
                .iter()
                .map(|def| NewShortcut::new(def.id, def.description).preferred_trigger(def.preferred_trigger))
                .collect();
            gs.bind_shortcuts(&session, &shortcuts, None, Default::default())
                .await?
                .response()?;
            anyhow::Ok(Shortcuts { gs, session })
        })
        .await
        .context("GlobalShortcuts portal unavailable")?;

    let mut stream = sc.gs.receive_activated().await?;
    while let Some(a) = stream.next().await {
        let Some(ev) = super::event(a.shortcut_id()) else { continue };
        let _ = tx.send(ev).await;
    }
    Ok(())
}

/// Current triggers as the portal describes them ("Ctrl+D"), by shortcut id.
pub async fn triggers() -> Result<std::collections::HashMap<String, String>> {
    let sc = SHORTCUTS.get().context("Global shortcuts portal not connected")?;
    let list = sc.gs.list_shortcuts(&sc.session, Default::default()).await?.response()?;
    Ok(list.shortcuts().iter().map(|s| (s.id().to_string(), s.trigger_description().to_string())).collect())
}

/// Open the portal's shortcut editor; false when it has none (Hyprland binds them in
/// hyprland.conf, older portals lack ConfigureShortcuts).
pub async fn configure() -> bool {
    match SHORTCUTS.get() {
        Some(sc) => sc.gs.configure_shortcuts(&sc.session, None, Default::default()).await.is_ok(),
        None => false,
    }
}

/// The keys are set in the desktop's own shortcut editor.
pub fn bind_help() -> BindHelp {
    BindHelp { text: "Your desktop manages these shortcuts.".into(), snippet: String::new(), desktop_editor: true }
}
