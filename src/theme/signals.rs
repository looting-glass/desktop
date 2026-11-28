//! The colours that carry meaning: red for danger, yellow for caution, green for good news. Every
//! theme uses the same ones, so red always means a deadly mod or a falling price. Only light and
//! dark backgrounds get their own set, since the bright dark-mode colours wash out on white.

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

impl Signals {
    pub fn on(background: Color) -> Self {
        if background.is_dark() { DARK } else { LIGHT }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signal colours mostly colour short text ("DEADLY", "▼ 12%"), so they need the 4.5:1
    /// WCAG asks of text, on the Looting Glass backgrounds.
    #[test]
    fn readable_on_their_background() {
        for (bg, signals) in [(Color::hex(0x1a1a24), DARK), (Color::hex(0xf3f1f7), LIGHT)] {
            assert_eq!(Signals::on(bg), signals);
            for c in [signals.danger, signals.caution, signals.positive] {
                assert!(c.contrast(bg) >= 4.5, "{c} on {bg}: {:.2}", c.contrast(bg));
            }
        }
    }
}
