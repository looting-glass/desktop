import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl

// CheckBox whose indicator follows the theme everywhere. Off KDE it draws its own box (Fusion
// renders dark-mode checkboxes on Windows as navy squares): field colour when unchecked, accent
// fill with a tick when checked. KDE keeps Breeze's checkbox.
CheckBox {
    id: control
    readonly property bool ownIndicator: Qt.platform.os !== "linux"

    Component.onCompleted: if (ownIndicator) indicator.opacity = 0

    Rectangle {
        visible: control.ownIndicator
        x: control.indicator.x
        y: control.indicator.y
        width: control.indicator.width
        height: control.indicator.height
        radius: 3
        color: control.checked ? control.palette.highlight : control.palette.base
        border.color: control.checked ? control.palette.highlight
                    : Qt.rgba(control.palette.text.r, control.palette.text.g, control.palette.text.b,
                              control.hovered ? 0.7 : 0.45)

        IconImage {
            anchors.centerIn: parent
            visible: control.checked
            source: "qrc:/lootingglass/icons/check.svg"
            color: control.palette.highlightedText
            sourceSize: Qt.size(parent.width - 2, parent.height - 2)
        }
    }
}
