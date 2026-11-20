import QtQuick

// The app's one "selected / enabled" look: a subtle accent tint. Used for checked filter rows
// and the selected settings page, so the accent reads the same everywhere. Put it inside the
// item as a background (anchors.fill: parent).
Rectangle {
    required property var palette
    property bool active: false
    property bool hovered: false

    radius: 4
    color: Qt.rgba(palette.highlight.r, palette.highlight.g, palette.highlight.b,
                   active ? 0.12 : hovered ? 0.06 : 0)
}
