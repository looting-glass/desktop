//! The Qt Quick style the windows are drawn with, chosen once before the first window opens.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// KDE's own (`org.kde.desktop`): follows the KDE colour scheme, but ignores most of a palette
    /// the app sets.
    Breeze,
    /// Qt's Fusion: draws everything from the palette, so it shows any theme.
    Fusion,
}

impl Style {
    /// Breeze only where it exists (Linux) and only for the System theme. `fusion_preview` is
    /// `LOOTINGGLASS_FUSION`, which previews Fusion on KDE with any theme.
    pub fn choose(linux: bool, follows_desktop: bool, fusion_preview: bool) -> Self {
        if linux && follows_desktop && !fusion_preview { Style::Breeze } else { Style::Fusion }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breeze_only_for_the_system_theme_on_linux() {
        assert_eq!(Style::choose(true, true, false), Style::Breeze);
        assert_eq!(Style::choose(true, false, false), Style::Fusion, "app themes need Fusion");
        assert_eq!(Style::choose(true, true, true), Style::Fusion, "LOOTINGGLASS_FUSION");
        assert_eq!(Style::choose(false, true, false), Style::Fusion, "no Breeze on Windows");
    }
}
