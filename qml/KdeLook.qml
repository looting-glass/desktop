import QtQml

// Off KDE, give a window the same colours as the KDE build in the README screenshots (dark,
// purple accent), whatever the system theme, so the app looks the same on every platform.
// Setting the window's palette also overrides Windows' per-control colours (navy checkboxes).
// On KDE the user's colour scheme applies. Use inside a window: KdeLook { target: palette }
QtObject {
    required property var target

    Component.onCompleted: {
        if (Qt.platform.os === "linux")
            return
        const p = target
        p.window = "#1a1a24"
        p.windowText = "#e2e2ec"
        p.base = "#13131a"
        p.alternateBase = "#222230"
        p.text = "#e2e2ec"
        p.button = "#262630"
        p.buttonText = "#e2e2ec"
        p.brightText = "#ffffff"
        p.highlight = "#8859a4"
        p.highlightedText = "#ffffff"
        p.link = "#b393d0"
        p.placeholderText = "#707d8a"
        p.toolTipBase = "#262630"
        p.toolTipText = "#e2e2ec"
        p.light = "#3f3f57"
        p.midlight = "#303043"
        p.mid = "#2d2d3e"
        p.dark = "#0d0d12"
        p.shadow = "#09090d"
        p.disabled.windowText = "#707d8a"
        p.disabled.text = "#707d8a"
        p.disabled.buttonText = "#707d8a"
    }
}
