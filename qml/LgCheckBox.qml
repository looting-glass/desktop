import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl

// CheckBox whose indicator follows the theme everywhere. Off KDE its indicator is replaced
// (Fusion draws dark-mode checkboxes on Windows as navy squares) with neutral tones from the
// text colour, matching the overlay's other state colours. KDE keeps Breeze's checkbox.
CheckBox {
    id: control

    Component.onCompleted: if (Qt.platform.os !== "linux") indicator = ownIndicator.createObject(control)

    Component {
        id: ownIndicator
        Rectangle {
            x: control.leftPadding
            y: control.topPadding + (control.availableHeight - height) / 2
            implicitWidth: 16
            implicitHeight: 16
            radius: 3
            readonly property color ink: control.palette.text
            color: control.checked ? Qt.rgba(ink.r, ink.g, ink.b, 0.22) : control.palette.base
            border.color: Qt.rgba(ink.r, ink.g, ink.b, control.checked || control.hovered ? 0.7 : 0.45)

            IconImage {
                anchors.centerIn: parent
                visible: control.checked
                source: "qrc:/lootingglass/icons/check.svg"
                color: control.palette.text
                sourceSize: Qt.size(12, 12)
            }
        }
    }
}
