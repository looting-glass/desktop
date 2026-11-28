//! Themes (Settings → Appearance): the desktop's own colours, a built-in preset in its light or
//! dark variant, or three custom colours. A theme resolves to `Looks`, what the windows draw
//! with while the system is in light mode and in dark mode, which QML picks from and applies.

mod color;
mod fade;
mod palette;
mod preset;
mod signals;
mod style;
mod swatches;

use crate::config::{AppearanceConfig, ColorMode};
use color::Color;
use fade::{FAINT, SECONDARY, Shade};
use palette::Palette;
use preset::{PRESETS, Preset};
use serde::Serialize;
use signals::Signals;
pub use style::Style;
use swatches::Swatches;

/// The preset that stands in for unknown theme names and unreadable custom colours.
const FALLBACK: &Preset = &PRESETS[0];

#[derive(Debug, PartialEq)]
pub enum Theme {
    /// The desktop's colours: the KDE colour scheme, or Windows' light or dark mode and accent.
    System,
    Preset(&'static Preset),
    Custom(Swatches),
}

impl Theme {
    /// The theme a config asks for. The config keeps what the user wrote, so a typo can be
    /// fixed by hand; meanwhile it shows as the Looting Glass preset.
    pub fn from_config(a: &AppearanceConfig) -> Self {
        match a.theme.as_str() {
            "system" => Theme::System,
            "custom" => Swatches::from_code(&a.colors).map_or(Theme::Preset(FALLBACK), Theme::Custom),
            id => Theme::Preset(preset::find(id).unwrap_or(FALLBACK)),
        }
    }

    /// Whether the windows keep the desktop's palette (and on KDE its Breeze style, which
    /// ignores most of a palette set by the app). The style is chosen before any window
    /// opens, so changing this takes a restart.
    pub fn follows_desktop(&self) -> bool {
        matches!(self, Theme::System)
    }

    /// `mode` picks a preset's variant; the desktop's colours and custom ones have only one.
    pub fn looks(&self, mode: ColorMode) -> Looks {
        Looks { light: self.look(false, mode), dark: self.look(true, mode) }
    }

    fn look(&self, system_dark: bool, mode: ColorMode) -> Look {
        let swatches = match self {
            Theme::System => None,
            Theme::Preset(p) => Some(p.swatches(match mode {
                ColorMode::System => system_dark,
                ColorMode::Light => false,
                ColorMode::Dark => true,
            })),
            Theme::Custom(s) => Some(*s),
        };
        let palette = swatches.as_ref().map(Palette::from_swatches);
        let text_on = palette.as_ref().map(|p| (p.window_text, p.window));
        let system_background = if system_dark { Color::BLACK } else { Color::WHITE };
        Look {
            selection_bar: swatches.map(|s| s.selection_bar),
            signals: Signals::on(swatches.map_or(system_background, |s| s.background)),
            secondary_text: SECONDARY.shade(text_on),
            faint_text: FAINT.shade(text_on),
            swatches: swatches.unwrap_or(FALLBACK.swatches(system_dark)),
            palette,
        }
    }
}

/// What the windows draw with, for each of the system's light and dark modes. A preset in a
/// fixed mode, or custom colours, look the same in both.
#[derive(Debug, Serialize)]
pub struct Looks {
    pub light: Look,
    pub dark: Look,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    /// `None`: the desktop's own palette.
    pub palette: Option<Palette>,
    /// `None`: the palette's highlight.
    pub selection_bar: Option<Color>,
    #[serde(flatten)]
    pub signals: Signals,
    /// Less important text (`fade::SECONDARY`).
    pub secondary_text: Shade,
    /// Least important text that must still be read (`fade::FAINT`).
    pub faint_text: Shade,
    /// The colours this look is made of, which Custom starts from when chosen. The System
    /// theme has none of its own and offers Looting Glass's.
    pub swatches: Swatches,
}

/// A theme as Settings offers it, with the colours its tile shows.
#[derive(Debug, Serialize)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    /// For the system's light and dark mode, like `Looks`; `None` for the desktop's colours and
    /// for custom colours not set yet.
    pub swatches: Option<Variants>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Variants {
    pub light: Swatches,
    pub dark: Swatches,
}

impl Variants {
    fn of(theme: &Theme, mode: ColorMode) -> Self {
        Variants { light: theme.look(false, mode).swatches, dark: theme.look(true, mode).swatches }
    }
}

/// Every theme, in the variant `a.mode` picks, with the custom colours `a.colors` holds.
fn choices(a: &AppearanceConfig) -> Vec<Choice> {
    let presets = PRESETS.iter().map(|p| Choice {
        id: p.id,
        label: p.label,
        swatches: Some(Variants::of(&Theme::Preset(p), a.mode)),
    });
    let custom = Swatches::from_code(&a.colors).ok().map(|s| Variants::of(&Theme::Custom(s), a.mode));
    std::iter::once(Choice { id: "system", label: "System", swatches: None })
        .chain(presets)
        .chain(std::iter::once(Choice { id: "custom", label: "Custom", swatches: custom }))
        .collect()
}

/// The Appearance page's view of settings not saved yet.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub looks: Looks,
    pub choices: Vec<Choice>,
    /// Why the custom colours can't be used, or `None`.
    pub error: Option<String>,
    /// Saving this needs a restart to take full effect (see `Theme::follows_desktop`).
    pub needs_restart: bool,
}

pub fn preview(a: &AppearanceConfig, running_follows_desktop: bool) -> Preview {
    let theme = Theme::from_config(a);
    let error = match a.theme.as_str() {
        "custom" => Swatches::from_code(&a.colors).err().map(|e| e.to_string()),
        _ => None,
    };
    Preview {
        looks: theme.looks(a.mode),
        choices: choices(a),
        error,
        needs_restart: theme.follows_desktop() != running_follows_desktop,
    }
}

/// The theme code for three `#rrggbb` colours, or why there's none.
pub fn code(background: &str, accent: &str, selection_bar: &str) -> anyhow::Result<String> {
    Ok(Swatches::from_colors(background, accent, selection_bar)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn appearance(theme: &str, mode: ColorMode, colors: &str) -> AppearanceConfig {
        AppearanceConfig { theme: theme.into(), mode, colors: colors.into() }
    }

    #[test]
    fn reads_the_theme_from_the_config() {
        let theme = |name, colors| Theme::from_config(&appearance(name, ColorMode::System, colors));
        assert_eq!(theme("system", ""), Theme::System);
        assert_eq!(theme("frost", ""), Theme::Preset(preset::find("frost").unwrap()));
        let custom = Swatches::from_code("lg1:102030,405060,708090").unwrap();
        assert_eq!(theme("custom", "lg1:102030,405060,708090"), Theme::Custom(custom));
        // Typos and broken colours fall back to Looting Glass.
        assert_eq!(theme("Frost", ""), Theme::Preset(FALLBACK));
        assert_eq!(theme("", ""), Theme::Preset(FALLBACK));
        assert_eq!(theme("custom", "lg1:nope"), Theme::Preset(FALLBACK));
        assert!(theme("system", "").follows_desktop());
        assert!(!theme("custom", "lg1:102030,405060,708090").follows_desktop());
    }

    #[test]
    fn mode_picks_the_preset_variant() {
        let lg = Theme::Preset(FALLBACK);
        let bg = |l: &Look| l.palette.as_ref().unwrap().window;
        let follows = lg.looks(ColorMode::System);
        assert_eq!(bg(&follows.light), FALLBACK.light.background);
        assert_eq!(bg(&follows.dark), FALLBACK.dark.background);
        let dark = lg.looks(ColorMode::Dark);
        assert_eq!((bg(&dark.light), bg(&dark.dark)), (FALLBACK.dark.background, FALLBACK.dark.background));
        let light = lg.looks(ColorMode::Light);
        assert_eq!((bg(&light.light), bg(&light.dark)), (FALLBACK.light.background, FALLBACK.light.background));
    }

    #[test]
    fn custom_colours_ignore_the_mode() {
        let s = Swatches::from_code("lg1:f0f0f0,2255ee,ee5522").unwrap();
        let looks = Theme::Custom(s).looks(ColorMode::Dark);
        for look in [&looks.light, &looks.dark] {
            assert_eq!(look.palette.as_ref().unwrap().window, s.background);
            assert_eq!(look.selection_bar, Some(s.selection_bar));
            assert_eq!(look.signals, Signals::on(s.background), "signals for the custom background");
            assert!(matches!(look.faint_text, Shade::Color(_)), "faded text worked out for it");
        }
    }

    #[test]
    fn system_theme_keeps_the_desktop_palette() {
        let looks = Theme::System.looks(ColorMode::Light);
        assert!(looks.light.palette.is_none() && looks.dark.palette.is_none());
        assert_eq!(looks.light.selection_bar, None);
        assert_eq!(looks.dark.signals, Signals::on(Color::BLACK), "the desktop's mode, not the preset mode");
        assert_eq!(looks.dark.swatches, FALLBACK.dark, "Custom starts from Looting Glass");
        assert_eq!(looks.dark.secondary_text, Shade::Fade(0.75), "QML fades the desktop's text");
    }

    #[test]
    fn looks_serialize_for_qml() {
        let json = serde_json::to_value(Theme::Preset(FALLBACK).looks(ColorMode::Dark)).unwrap();
        let dark = &json["dark"];
        assert_eq!(dark["palette"]["window"], "#1a1a24");
        assert_eq!(dark["palette"]["highlight"], "#8859a4");
        assert!(dark["palette"]["placeholderText"].is_string());
        assert!(dark["palette"]["disabled"]["text"].is_string());
        assert!(dark["secondaryText"].is_string());
        assert_eq!(dark["selectionBar"], "#8859a4");
        assert_eq!(dark["danger"], "#e06c6c");
        assert_eq!(dark["swatches"]["selectionBar"], "#8859a4");
        let system = serde_json::to_value(Theme::System.looks(ColorMode::System)).unwrap();
        assert!(system["light"]["palette"].is_null());
    }

    #[test]
    fn preview_reports_bad_colours_and_restarts() {
        let p = preview(&appearance("custom", ColorMode::System, "lg1:12"), true);
        assert!(p.error.unwrap().contains("has 3 colours"));
        assert!(p.needs_restart, "leaving the desktop's colours");
        let p = preview(&appearance("exile", ColorMode::Dark, "lg1:12"), false);
        assert_eq!(p.error, None, "custom colours only matter for Custom");
        assert!(!p.needs_restart, "preset to preset applies at once");
        assert!(preview(&appearance("system", ColorMode::System, ""), false).needs_restart);
    }

    #[test]
    fn codes_from_colour_fields() {
        assert_eq!(code("#1A1A24", "8859a4", " #8859a4 ").unwrap(), "lg1:1a1a24,8859a4,8859a4");
        assert!(code("#1a1a24", "#8859a", "#8859a4").unwrap_err().to_string().contains("\"#8859a\""));
    }

    #[test]
    fn choices_list_system_presets_and_custom() {
        let list = choices(&appearance("system", ColorMode::Dark, ""));
        let ids: Vec<_> = list.iter().map(|c| c.id).collect();
        assert_eq!(ids, ["system", "looting-glass", "exile", "frost", "verdant", "custom"]);
        assert_eq!(list[0].swatches, None, "the desktop's colours aren't known here");
        let lg = list[1].swatches.as_ref().unwrap();
        assert_eq!((lg.light, lg.dark), (FALLBACK.dark, FALLBACK.dark), "the variant the mode picks");
        assert_eq!(list[5].swatches, None, "no custom colours yet");

        let custom = choices(&appearance("exile", ColorMode::System, "lg1:102030,405060,708090"));
        let s = Swatches::from_code("lg1:102030,405060,708090").unwrap();
        assert_eq!(custom[5].swatches, Some(Variants { light: s, dark: s }));
        let lg = custom[1].swatches.as_ref().unwrap();
        assert_eq!((lg.light, lg.dark), (FALLBACK.light, FALLBACK.dark));
    }
}
