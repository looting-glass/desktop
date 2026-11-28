import QtQuick
import dev.lootingglass

// A window's colours for the theme chosen under Settings → Appearance (Backend.themeLooks): the
// palette, the faded text colours, the selection bar and the signal colours, for the system's
// current light or dark mode. With the System theme the palette stays the desktop's own; the
// rest still comes from here. Setting the window's palette also overrides Windows' per-control
// colours. Use inside a window: AppTheme { id: theme; target: win.palette }
QtObject {
    required property var target

    readonly property var looks: JSON.parse(Backend.themeLooks)
    readonly property var look: Qt.styleHints.colorScheme === Qt.ColorScheme.Dark ? looks.dark : looks.light
    readonly property var palette: look.palette

    // The desktop's icon theme (KDE) is drawn for its colour scheme, so only Breeze, which keeps
    // that scheme, uses it; Fusion gets the bundled monochrome icons, tinted with the text colour.
    readonly property bool desktopIcons: Backend.breezeStyle
    // Less important and least important text. The desktop's text is faded by the amounts
    // `theme::palette` starts from; themes work theirs out with a minimum contrast.
    readonly property color secondaryText: palette ? palette.secondaryText : fade(0.75)
    readonly property color faintText: palette ? palette.faintText : fade(0.55)
    readonly property color selectionBar: look.selectionBar || target.highlight
    readonly property color danger: look.danger
    readonly property color caution: look.caution
    readonly property color positive: look.positive

    // The QPalette roles a theme sets (`theme::Palette` has a few more colours of its own).
    readonly property var roles: ["window", "windowText", "base", "alternateBase", "text", "button",
        "buttonText", "brightText", "highlight", "highlightedText", "link", "placeholderText",
        "toolTipBase", "toolTipText", "light", "midlight", "mid", "dark", "shadow"]

    onLookChanged: apply()
    Component.onCompleted: apply()

    function fade(amount) {
        const t = target.windowText
        return Qt.rgba(t.r, t.g, t.b, amount)
    }

    function apply() {
        if (!palette)
            return
        for (const role of roles)
            target[role] = palette[role]
        target.disabled.windowText = palette.disabledText
        target.disabled.text = palette.disabledText
        target.disabled.buttonText = palette.disabledText
    }
}
