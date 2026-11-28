//! The three colours a theme is made of, and the theme code that shares them as one line of
//! text (`lg1:1a1a24,8859a4,8859a4`): copied from Settings, pasted into someone else's, and
//! stored as the custom colours in the config.

use super::color::Color;
use anyhow::{Context, bail};
use serde::Serialize;
use std::fmt;

/// Versions the code's format, so swatches can be added later without misreading old codes.
const PREFIX: &str = "lg1:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Swatches {
    /// Windows, the overlay card, fields and buttons; every other background shade and the
    /// text colour are worked out from it.
    pub background: Color,
    /// The item name, chips and the tint behind selected rows.
    pub accent: Color,
    /// The thin bar on the left of a selected row.
    pub selection_bar: Color,
}

impl Swatches {
    /// Reads a theme code, with a plain-language error for anything else.
    pub fn from_code(code: &str) -> anyhow::Result<Self> {
        let Some(list) = code.trim().strip_prefix(PREFIX) else {
            bail!("That isn't a Looting Glass theme code. Theme codes start with \"{PREFIX}\".");
        };
        let colors = list
            .split(',')
            .map(|c| Color::parse(c).with_context(|| format!("\"{}\" isn't a colour like #8859a4.", c.trim())))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let [background, accent, selection_bar] = colors[..] else {
            bail!("A theme code has 3 colours; this one has {}.", colors.len());
        };
        Ok(Self { background, accent, selection_bar })
    }
}

/// The theme code: `lg1:` and the colours without their `#`.
impl fmt::Display for Swatches {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hex = |c: Color| c.to_string().split_off(1);
        write!(f, "{PREFIX}{},{},{}", hex(self.background), hex(self.accent), hex(self.selection_bar))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: Swatches = Swatches {
        background: Color::hex(0x1a1a24),
        accent: Color::hex(0x8859a4),
        selection_bar: Color::hex(0xb393d0),
    };

    #[test]
    fn code_round_trips() {
        assert_eq!(SAMPLE.to_string(), "lg1:1a1a24,8859a4,b393d0");
        assert_eq!(Swatches::from_code(&SAMPLE.to_string()).unwrap(), SAMPLE);
    }

    #[test]
    fn code_tolerates_spaces_hashes_and_case() {
        let pasted = "  lg1: #1A1A24, 8859a4 ,#B393D0\n";
        assert_eq!(Swatches::from_code(pasted).unwrap(), SAMPLE);
    }

    #[test]
    fn bad_codes_say_what_is_wrong() {
        let err = |code| Swatches::from_code(code).unwrap_err().to_string();
        assert!(err("").contains("start with \"lg1:\""));
        assert!(err("1a1a24,8859a4,b393d0").contains("start with"));
        assert!(err("lg2:1a1a24,8859a4,b393d0").contains("start with"));
        assert!(err("lg1:1a1a24,8859a4").contains("has 3 colours; this one has 2"));
        assert!(err("lg1:1a1a24,8859a4,b393d0,ffffff").contains("this one has 4"));
        assert!(err("lg1:1a1a24,purple,b393d0").contains("\"purple\" isn't a colour"));
        assert!(err("lg1:").contains("\"\" isn't a colour"));
    }
}
