//! A full Qt palette worked out from a theme's swatches, so users pick three colours and every
//! role stays consistent and readable: shades of the background for fields and borders, text
//! that contrasts with it, a link colour from the accent.

use super::color::Color;
use super::swatches::Swatches;
use serde::Serialize;

/// The `QPalette` roles the windows set, named as QML's `palette` names them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub window: Color,
    pub window_text: Color,
    pub base: Color,
    pub alternate_base: Color,
    pub text: Color,
    pub button: Color,
    pub button_text: Color,
    pub bright_text: Color,
    pub highlight: Color,
    pub highlighted_text: Color,
    pub link: Color,
    pub placeholder_text: Color,
    pub tool_tip_base: Color,
    pub tool_tip_text: Color,
    pub light: Color,
    pub midlight: Color,
    pub mid: Color,
    pub dark: Color,
    pub shadow: Color,
    /// Text of disabled controls (`palette.disabled.text` and friends).
    pub disabled_text: Color,
    /// Less important text: section headers, counts, listing ages, status lines.
    pub secondary_text: Color,
    /// Least important text that must still be read: mod tags, roll ranges, unticked filters.
    pub faint_text: Color,
}

/// How far `secondary_text` and `faint_text` lean towards the full text colour, unless the
/// background needs more for them to stay readable. AppTheme.qml fades the System theme's text
/// by the same amounts.
const SECONDARY: Fade = Fade { amount: 0.75, min_contrast: 6.0 };
const FAINT: Fade = Fade { amount: 0.55, min_contrast: 4.5 };

struct Fade {
    amount: f32,
    min_contrast: f32,
}

impl Fade {
    /// Text faded towards the background. A fixed fade (like the overlay's old `opacity: 0.5`)
    /// that reads fine on dark backgrounds washes out on light ones, so it fades only as far as
    /// the contrast allows.
    fn apply(&self, text: Color, bg: Color) -> Color {
        let mut amount = self.amount;
        while amount < 1.0 && bg.mix(text, amount).contrast(bg) < self.min_contrast {
            amount += 0.05;
        }
        bg.mix(text, amount.min(1.0))
    }
}

impl Palette {
    pub fn from_swatches(s: &Swatches) -> Self {
        let bg = s.background;
        let dark = bg.is_dark();
        // Near-white or near-black, tinted with the background so it doesn't glare; plain white
        // or black on mid-tone backgrounds, where the tint would cost too much contrast.
        let tinted = if dark { bg.mix(Color::WHITE, 0.88) } else { bg.mix(Color::BLACK, 0.85) };
        let text = if tinted.contrast(bg) >= 7.0 { tinted } else { readable_on(bg) };
        let towards_text = |amount| bg.mix(text, amount);
        let muted = towards_text(0.45);
        let button = towards_text(0.07);
        Palette {
            window: bg,
            window_text: text,
            // Fields sit a little deeper than the window: darker on dark, whiter on light.
            base: if dark { bg.mix(Color::BLACK, 0.25) } else { bg.mix(Color::WHITE, 0.6) },
            alternate_base: towards_text(0.05),
            text,
            button,
            button_text: text,
            bright_text: Color::WHITE,
            highlight: s.accent,
            highlighted_text: readable_on(s.accent),
            // A link has to stand out from the accent-coloured text around it.
            link: if dark { s.accent.mix(Color::WHITE, 0.35) } else { s.accent.mix(Color::BLACK, 0.2) },
            placeholder_text: FAINT.apply(text, bg),
            tool_tip_base: button,
            tool_tip_text: text,
            light: if dark { towards_text(0.18) } else { bg.mix(Color::WHITE, 0.8) },
            midlight: towards_text(0.11),
            mid: towards_text(0.09),
            dark: bg.mix(Color::BLACK, if dark { 0.5 } else { 0.35 }),
            shadow: bg.mix(Color::BLACK, 0.65),
            disabled_text: muted,
            secondary_text: SECONDARY.apply(text, bg),
            faint_text: FAINT.apply(text, bg),
        }
    }
}

fn readable_on(c: Color) -> Color {
    if c.is_dark() { Color::WHITE } else { Color::BLACK }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::preset::PRESETS;

    fn swatches(background: u32, accent: u32) -> Swatches {
        let accent = Color::hex(accent);
        Swatches { background: Color::hex(background), accent, selection_bar: accent }
    }

    #[test]
    fn text_is_readable_on_every_background() {
        let mut all: Vec<Swatches> = PRESETS.iter().flat_map(|p| [p.light, p.dark]).collect();
        // Whatever users pick, including mid tones. (Greys from #777 to #888 can't reach 4.5:1
        // with any text colour.)
        all.extend([0x000000, 0xffffff, 0x6a6a6a, 0x999999, 0x2255ee, 0xffee00].map(|bg| swatches(bg, 0x8859a4)));
        for s in all {
            let p = Palette::from_swatches(&s);
            for (role, bg) in [("window", p.window), ("base", p.base), ("button", p.button)] {
                let c = p.text.contrast(bg);
                assert!(c >= 4.5, "text on {role} {bg} (from {}): {c:.2}", s.background);
            }
            assert!(p.highlighted_text.contrast(p.highlight) >= 4.5, "on accent {}", s.accent);
            assert!(p.secondary_text.contrast(p.window) >= 6.0_f32.min(p.text.contrast(p.window)));
            assert!(p.faint_text.contrast(p.window) >= 4.5, "faint text on {}", s.background);
        }
    }

    #[test]
    fn fades_only_as_far_as_the_background_allows() {
        let text = Color::hex(0xe3e3e4);
        let dark = Color::hex(0x1a1a24);
        // Dark backgrounds take the plain fade.
        assert_eq!(FAINT.apply(text, dark), dark.mix(text, FAINT.amount));
        // Light ones need less fading to keep the contrast.
        let (ink, light) = (Color::hex(0x2a2826), Color::hex(0xf7f2ee));
        let faint = FAINT.apply(ink, light);
        assert!(faint.contrast(light) >= 4.5);
        assert_ne!(faint, light.mix(ink, FAINT.amount));
        assert!(SECONDARY.apply(ink, light).contrast(light) > faint.contrast(light), "secondary stands out more");
    }

    #[test]
    fn shades_follow_the_background() {
        let dark = Palette::from_swatches(&swatches(0x1a1a24, 0x8859a4));
        assert_eq!(dark.window, Color::hex(0x1a1a24));
        assert_eq!(dark.highlight, Color::hex(0x8859a4));
        assert_eq!(dark.highlighted_text, Color::WHITE);
        assert!(dark.text.contrast(Color::WHITE) < 1.5, "near-white text on dark");
        assert!(dark.base.contrast(Color::BLACK) < dark.window.contrast(Color::BLACK), "fields are deeper");

        let light = Palette::from_swatches(&swatches(0xf3f1f7, 0x7a4a96));
        assert!(light.text.contrast(Color::BLACK) < 1.5, "near-black text on light");
        assert!(light.base.contrast(Color::WHITE) < light.window.contrast(Color::WHITE), "fields are whiter");
    }
}
