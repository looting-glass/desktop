//! An sRGB colour as themes use it: written as `#rrggbb`, mixed and compared for contrast.

use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    pub const WHITE: Color = Color::hex(0xffffff);
    pub const BLACK: Color = Color::hex(0x000000);

    /// `Color::hex(0x8859a4)`, for colours written in the code.
    pub const fn hex(rgb: u32) -> Self {
        Self { r: (rgb >> 16) as u8, g: (rgb >> 8) as u8, b: rgb as u8 }
    }

    /// `#8859a4` or `8859a4`, in either case.
    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(hex, 16).ok().map(Self::hex)
    }

    /// The colour `amount` (0 to 1) of the way towards `other`.
    pub fn mix(self, other: Color, amount: f32) -> Self {
        let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        Self { r: channel(self.r, other.r), g: channel(self.g, other.g), b: channel(self.b, other.b) }
    }

    /// Relative luminance as WCAG defines it: 0 for black, 1 for white.
    fn luminance(self) -> f32 {
        let linear = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    /// WCAG contrast ratio, from 1 (same colour) to 21 (black on white).
    pub fn contrast(self, other: Color) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Whether white text reads better on this colour than black text.
    pub fn is_dark(self) -> bool {
        self.contrast(Color::WHITE) > self.contrast(Color::BLACK)
    }

    /// White or black, whichever reads better on this colour.
    pub fn contrasting(self) -> Color {
        if self.is_dark() { Color::WHITE } else { Color::BLACK }
    }

    /// This colour, moved towards `target` in 5% steps until it has `min_contrast` against `bg`;
    /// `target` itself when no step gets there.
    pub fn until_readable(self, target: Color, bg: Color, min_contrast: f32) -> Color {
        (0..=20)
            .map(|step| self.mix(target, step as f32 / 20.0))
            .find(|c| c.contrast(bg) >= min_contrast)
            .unwrap_or(target)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_writes_hex() {
        assert_eq!(Color::parse("#8859a4"), Some(Color::hex(0x8859a4)));
        assert_eq!(Color::parse(" 8859A4 "), Some(Color::hex(0x8859a4)));
        assert_eq!(Color::hex(0x0a0b0c).to_string(), "#0a0b0c");
        for bad in ["", "#", "#8859a", "#8859a4f", "#88g9a4", "rgb(1,2,3)", "#+859a4"] {
            assert_eq!(Color::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn mixes_towards_the_other_colour() {
        let c = Color::hex(0x204060);
        assert_eq!(c.mix(Color::WHITE, 0.0), c);
        assert_eq!(c.mix(Color::WHITE, 1.0), Color::WHITE);
        assert_eq!(Color::BLACK.mix(Color::WHITE, 0.5), Color::hex(0x808080));
    }

    #[test]
    fn moves_only_as_far_as_readable() {
        let bg = Color::hex(0x1a1a24);
        let red = Color::hex(0xe06c6c);
        assert_eq!(red.until_readable(Color::WHITE, bg, 4.5), red, "already readable");
        let dim = Color::hex(0x603030);
        let lifted = dim.until_readable(Color::WHITE, bg, 4.5);
        assert!(lifted.contrast(bg) >= 4.5 && lifted != Color::WHITE);
        // #777 can't reach 7:1 with anything; the target is the best there is.
        assert_eq!(dim.until_readable(Color::WHITE, Color::hex(0x777777), 7.0), Color::WHITE);
        assert_eq!(bg.contrasting(), Color::WHITE);
        assert_eq!(Color::hex(0xf3f1f7).contrasting(), Color::BLACK);
    }

    #[test]
    fn contrast_and_darkness() {
        assert!((Color::BLACK.contrast(Color::WHITE) - 21.0).abs() < 0.01);
        assert!((Color::WHITE.contrast(Color::WHITE) - 1.0).abs() < 0.01);
        assert!(Color::hex(0x1a1a24).is_dark());
        assert!(!Color::hex(0xf3f1f7).is_dark());
        assert!(!Color::hex(0xe0b84a).is_dark(), "the caution yellow takes black text");
    }
}
