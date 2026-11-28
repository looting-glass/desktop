//! The Qt Quick style the windows are drawn with, chosen once before the first window opens.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// KDE's own (`org.kde.desktop`): follows the KDE colour scheme, but ignores most of a palette
    /// the app sets.
    Breeze,
    /// Fusion, or another style the user forced: draws from the palette, so it shows any theme.
    Fusion,
}

const BREEZE: &str = "org.kde.desktop";

impl Style {
    /// Breeze only where it exists (Linux) and only for the System theme. `forced` is a style
    /// set from outside (`QT_QUICK_CONTROLS_STYLE`, or Fusion for `LOOTINGGLASS_FUSION`), which
    /// wins, so the windows know which style they really get.
    pub fn choose(linux: bool, follows_desktop: bool, forced: Option<&str>) -> Self {
        let breeze = match forced {
            Some(style) => style == BREEZE,
            None => linux && follows_desktop,
        };
        if breeze { Style::Breeze } else { Style::Fusion }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breeze_only_for_the_system_theme_on_linux() {
        assert_eq!(Style::choose(true, true, None), Style::Breeze);
        assert_eq!(Style::choose(true, false, None), Style::Fusion, "app themes need Fusion");
        assert_eq!(Style::choose(false, true, None), Style::Fusion, "no Breeze on Windows");
    }

    #[test]
    fn a_forced_style_wins() {
        assert_eq!(Style::choose(true, true, Some("Fusion")), Style::Fusion, "LOOTINGGLASS_FUSION");
        assert_eq!(Style::choose(true, true, Some("Material")), Style::Fusion);
        assert_eq!(Style::choose(true, false, Some(BREEZE)), Style::Breeze, "Breeze forced by hand");
    }
}
