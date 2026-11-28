//! Hyprland: the portal registers our shortcuts, but Hyprland doesn't let apps choose their
//! keys, so the user binds them in hyprland.conf.

use super::ShortcutDef;
use crate::platform::BindHelp;
use crate::platform::linux::APP_ID;

/// A `bind = …` line for each shortcut, with its preferred key or a placeholder to replace.
pub fn bind_help() -> BindHelp {
    BindHelp {
        text: "Hyprland binds global shortcuts in hyprland.conf. Add a line for each one you want, with your own keys:".into(),
        snippet: super::snippet(bind_line),
        desktop_editor: false,
    }
}

fn bind_line(def: &ShortcutDef) -> String {
    format!("bind = {}, global, {APP_ID}:{}", keys(def.preferred_trigger), def.id)
}

/// Hyprland's "MODS, key" for a portal trigger: "CTRL+D" is "CTRL, D", "CTRL+SHIFT+F7" is
/// "CTRL SHIFT, F7". Without one, a placeholder key the user replaces.
fn keys(trigger: Option<&str>) -> String {
    match trigger.map(|t| t.rsplit_once('+').unwrap_or(("", t))) {
        Some((mods, key)) => format!("{}, {key}", mods.replace('+', " ")),
        None => ", KEY".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_triggers_become_hyprland_keys() {
        assert_eq!(keys(Some("CTRL+D")), "CTRL, D");
        assert_eq!(keys(Some("CTRL+SHIFT+F7")), "CTRL SHIFT, F7");
        assert_eq!(keys(Some("F7")), ", F7");
        assert_eq!(keys(None), ", KEY");
    }

    #[test]
    fn price_check_gets_its_usual_key() {
        assert_eq!(
            bind_help().snippet.lines().next(),
            Some("bind = CTRL, D, global, dev.lootingglass.LootingGlass:price-check   # Price check hovered item")
        );
    }
}
