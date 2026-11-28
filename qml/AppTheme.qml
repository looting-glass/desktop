import QtQuick
import dev.lootingglass

// A window's colours for the theme chosen under Settings → Appearance (Backend.themeLooks): the
// palette, the faded text colours, the selection bar and the signal colours, for the system's
// current light or dark mode. With the System theme the palette stays the desktop's own; the
// rest still comes from here. Setting the window's palette also overrides Windows' per-control
// colours. Use inside a window: AppTheme { id: theme; target: win.palette }
QtObject {
    required property var target

    // Whether the system is in dark mode. Where it doesn't say (Qt reports Unknown, e.g. on
    // Hyprland without a colour-scheme setting), the desktop's own window colour decides.
    readonly property SystemPalette desktop: SystemPalette {}
    readonly property bool dark: Qt.styleHints.colorScheme === Qt.ColorScheme.Unknown
        ? desktop.window.hslLightness < 0.5
        : Qt.styleHints.colorScheme === Qt.ColorScheme.Dark

    readonly property var looks: JSON.parse(Backend.themeLooks)
    readonly property var look: dark ? looks.dark : looks.light
    readonly property var palette: look.palette

    // The desktop's icon theme (KDE) is drawn for its colour scheme, so only Breeze, which keeps
    // that scheme, uses it; Fusion gets the bundled monochrome icons, tinted with the text colour.
    readonly property bool desktopIcons: Backend.breezeStyle
    readonly property color secondaryText: shade(look.secondaryText)
    readonly property color faintText: shade(look.faintText)
    readonly property color selectionBar: look.selectionBar || target.highlight
    readonly property color danger: look.danger
    readonly property color caution: look.caution
    readonly property color positive: look.positive

    onLookChanged: apply()
    Component.onCompleted: apply()

    // A theme's faded text colour (`theme::fade::Shade`), or for the System theme how far to fade
    // the desktop's text.
    function shade(s) {
        if (typeof s !== "number")
            return s
        const t = target.windowText
        return Qt.rgba(t.r, t.g, t.b, s)
    }

    // Sets every role the theme sends (`theme::Palette`), the disabled ones included.
    function apply() {
        if (!palette)
            return
        for (const role in palette) {
            if (role === "disabled") {
                for (const disabledRole in palette.disabled)
                    target.disabled[disabledRole] = palette.disabled[disabledRole]
            } else {
                target[role] = palette[role]
            }
        }
    }
}
