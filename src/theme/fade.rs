//! Faded text: less important text (section headers, counts, listing ages, status lines) and the
//! least important text that must still be read (mod tags, roll ranges, unticked filters).

use super::color::Color;
use serde::Serialize;

/// How far faded text leans towards the full text colour, unless the background needs more for
/// it to stay readable.
pub struct Fade {
    amount: f32,
    min_contrast: f32,
}

pub const SECONDARY: Fade = Fade { amount: 0.75, min_contrast: 6.0 };
pub const FAINT: Fade = Fade { amount: 0.55, min_contrast: 4.5 };

impl Fade {
    /// Text faded towards the background. A fixed fade (like the overlay's old `opacity: 0.5`)
    /// that reads fine on dark backgrounds washes out on light ones, so it fades only as far as
    /// the contrast allows.
    pub fn apply(&self, text: Color, bg: Color) -> Color {
        bg.mix(text, self.amount).until_readable(text, bg, self.min_contrast)
    }

    /// The shade QML draws with: worked out from a theme's text and background, or for the
    /// desktop's colours (System theme), which only QML knows, the plain fade.
    pub fn shade(&self, text_on: Option<(Color, Color)>) -> Shade {
        match text_on {
            Some((text, bg)) => Shade::Color(self.apply(text, bg)),
            None => Shade::Fade(self.amount),
        }
    }
}

/// A faded text colour, or how far QML fades the desktop's text towards its background.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Shade {
    Color(Color),
    Fade(f32),
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn shades_for_qml() {
        let (text, bg) = (Color::hex(0xe3e3e4), Color::hex(0x1a1a24));
        assert_eq!(SECONDARY.shade(Some((text, bg))), Shade::Color(SECONDARY.apply(text, bg)));
        assert_eq!(FAINT.shade(None), Shade::Fade(0.55));
        assert_eq!(serde_json::to_value(FAINT.shade(None)).unwrap(), 0.55_f32 as f64);
        assert_eq!(serde_json::to_value(FAINT.shade(Some((text, bg)))).unwrap().as_str().map(|s| s.len()), Some(7));
    }
}
