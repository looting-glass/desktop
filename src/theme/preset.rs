//! The built-in themes, each with a light and a dark variant.

use super::color::Color;
use super::swatches::Swatches;

#[derive(Debug, PartialEq)]
pub struct Preset {
    /// Stored in the config (`theme = "looting-glass"`).
    pub id: &'static str,
    pub label: &'static str,
    pub light: Swatches,
    pub dark: Swatches,
}

impl Preset {
    pub fn swatches(&self, dark: bool) -> Swatches {
        if dark { self.dark } else { self.light }
    }
}

/// Swatches whose selection bar is the accent.
const fn swatches(background: u32, accent: u32) -> Swatches {
    swatches_with_bar(background, accent, accent)
}

const fn swatches_with_bar(background: u32, accent: u32, selection_bar: u32) -> Swatches {
    Swatches { background: Color::hex(background), accent: Color::hex(accent), selection_bar: Color::hex(selection_bar) }
}

/// The first is the default: Looting Glass's own purple, the look of the screenshots.
pub const PRESETS: [Preset; 4] = [
    Preset {
        id: "looting-glass",
        label: "Looting Glass",
        light: swatches(0xf3f1f7, 0x7a4a96),
        dark: swatches(0x1a1a24, 0x8859a4),
    },
    // Path of Exile's own colours: old gold on dark stone, with the logo's blood red as the
    // selection bar; limestone and a deeper bronze in light mode.
    Preset {
        id: "exile",
        label: "Exile",
        light: swatches_with_bar(0xefe9dd, 0x7d5f22, 0x8e1e20),
        dark: swatches_with_bar(0x1d1b17, 0xc8a86b, 0xa3262a),
    },
    Preset { id: "frost", label: "Frost", light: swatches(0xeef2f7, 0x2f6aa8), dark: swatches(0x161b22, 0x4a8fd4) },
    Preset { id: "verdant", label: "Verdant", light: swatches(0xeff4ef, 0x3a7a45), dark: swatches(0x171d19, 0x57a865) },
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_presets_by_id() {
        assert_eq!(find("frost").map(|p| p.label), Some("Frost"));
        assert_eq!(find("Frost"), None);
        assert_eq!(find(""), None);
    }

    #[test]
    fn variants_are_what_they_say() {
        for p in &PRESETS {
            assert!(p.dark.background.is_dark(), "{} dark", p.id);
            assert!(!p.light.background.is_dark(), "{} light", p.id);
        }
    }

    /// The accent colours text (the item name) and small marks, so it needs at least the 3:1
    /// WCAG asks of large text and UI parts.
    #[test]
    fn accents_stand_out_from_their_background() {
        for p in &PRESETS {
            for s in [p.light, p.dark] {
                let contrast = s.accent.contrast(s.background);
                assert!(contrast >= 3.0, "{} {}: {contrast:.2}", p.id, s.background);
            }
        }
    }
}
