import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl

// CheckBox whose indicator follows the theme everywhere. Off KDE its indicator is replaced
// (Fusion draws dark-mode checkboxes on Windows as navy squares): field colour with a soft
// outline when unchecked, accent fill with a tick when checked. KDE keeps Breeze's checkbox.
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
            color: control.checked ? control.palette.highlight : control.palette.base
            border.color: control.checked ? control.palette.highlight
                        : Qt.rgba(control.palette.text.r, control.palette.text.g, control.palette.text.b,
                                  control.hovered ? 0.7 : 0.45)

            IconImage {
                anchors.centerIn: parent
                visible: control.checked
                source: "qrc:/lootingglass/icons/check.svg"
                color: control.palette.highlightedText
                sourceSize: Qt.size(12, 12)
            }
        }
    }
}
