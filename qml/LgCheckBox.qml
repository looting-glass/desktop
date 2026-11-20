import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl

// CheckBox that looks like KDE's Breeze checkbox everywhere. Off KDE its indicator is replaced
// (Fusion draws dark-mode checkboxes on Windows as navy squares): a field-coloured box with a
// soft outline and a tick. KDE keeps Breeze's own checkbox.
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
            color: control.palette.base
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
