import QtQuick

// The app's one "selected / enabled" look: a medium accent tint with a solid accent bar on the
// left. Used for checked filter rows and the settings sidebar, so the accent reads the same
// everywhere. Put it inside the item as a background (anchors.fill: parent; z: -1).
Rectangle {
    required property var palette
    property bool active: false
    property bool hovered: false

    // Keep the accent's hue and full saturation and move only its lightness part of the way to the
    // background's: a transparent overlay would grey it out on dark themes.
    function shade(amount) {
        const h = palette.highlight, w = palette.window
        return Qt.hsla(h.hslHue, h.hslSaturation, w.hslLightness + (h.hslLightness - w.hslLightness) * amount, 1)
    }

    radius: 4
    color: active ? shade(0.75) : hovered ? shade(0.25) : "transparent"

    Rectangle {
        visible: parent.active
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom; margins: 4 }
        width: 3
        radius: 1.5
        color: parent.palette.highlight
    }
}
