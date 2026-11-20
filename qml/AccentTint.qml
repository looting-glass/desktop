import QtQuick

// The app's one "selected / enabled" look: a subtle tint of the accent with a solid accent bar
// on the left. Used for checked filter rows and the selected settings page, so state reads the
// same everywhere. Put it inside the item as a background (anchors.fill: parent).
Rectangle {
    // KDE: the theme's highlight. Elsewhere: a neutral tone from the text colour.
    required property color accent
    property bool active: false
    property bool hovered: false

    radius: 4
    color: Qt.rgba(accent.r, accent.g, accent.b, active ? 0.12 : hovered ? 0.06 : 0)

    Rectangle {
        visible: parent.active
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom; margins: 4 }
        width: 3
        radius: 1.5
        color: parent.accent
    }
}
