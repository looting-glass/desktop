import QtQuick

// The app's one "selected / enabled" look: a subtle accent tint with a solid accent bar on the
// left. Used for checked filter rows and the selected settings page, so the accent reads the
// same everywhere. Put it inside the item as a background (anchors.fill: parent).
Rectangle {
    required property var colors
    property bool active: false
    property bool hovered: false

    radius: 4
    color: Qt.rgba(colors.highlight.r, colors.highlight.g, colors.highlight.b,
                   active ? 0.12 : hovered ? 0.06 : 0)

    Rectangle {
        visible: parent.active
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom; margins: 4 }
        width: 3
        radius: 1.5
        color: parent.colors.highlight
    }
}
