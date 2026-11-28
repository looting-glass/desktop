//! The colours that carry meaning: red for danger, yellow for caution, green for good news. Every
//! theme uses the same ones, so red always means a deadly mod or a falling price. Light and dark
//! backgrounds get their own set, since the bright dark-mode colours wash out on white, and on a
//! custom background in between they're lightened or darkened until they read.

use super::color::Color;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Signals {
    /// Errors, corrupted items, deadly map mods, falling prices.
    pub danger: Color,
    /// Annoying map mods, shortcut conflicts, rate-limit warnings.
    pub caution: Color,
    /// Rising prices.
    pub positive: Color,
}

const DARK: Signals =
    Signals { danger: Color::hex(0xe06c6c), caution: Color::hex(0xe0b84a), positive: Color::hex(0x6cc070) };

const LIGHT: Signals =
    Signals { danger: Color::hex(0xc0392b), caution: Color::hex(0x8f6200), positive: Color::hex(0x1f7f3a) };

/// Signal colours mostly colour short text ("DEADLY", "▼ 12%"), so they get the 4.5:1 WCAG asks
/// of text.
const MIN_CONTRAST: f32 = 4.5;

impl Signals {
    pub fn on(background: Color) -> Self {
        let set = if background.is_dark() { DARK } else { LIGHT };
        let readable = |c: Color| c.until_readable(background.contrasting(), background, MIN_CONTRAST);
        Signals { danger: readable(set.danger), caution: readable(set.caution), positive: readable(set.positive) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(s: Signals) -> [Color; 3] {
        [s.danger, s.caution, s.positive]
    }

    #[test]
    fn looting_glass_backgrounds_keep_the_designed_colours() {
        assert_eq!(Signals::on(Color::hex(0x1a1a24)), DARK);
        assert_eq!(Signals::on(Color::hex(0xf3f1f7)), LIGHT);
    }

    #[test]
    fn readable_on_any_background() {
        // Mid tones a custom theme might pick, where the designed sets fall short.
        for bg in [0x1a1a24, 0xf3f1f7, 0x5a5a5a, 0x6a6a6a, 0x999999, 0x2255ee, 0x8859a4, 0xffee00] {
            let bg = Color::hex(bg);
            for c in all(Signals::on(bg)) {
                assert!(c.contrast(bg) >= MIN_CONTRAST, "{c} on {bg}: {:.2}", c.contrast(bg));
            }
        }
    }
}
