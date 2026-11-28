import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import dev.lootingglass

// CheckBox that looks like KDE's Breeze checkbox everywhere. Under Fusion its indicator is
// replaced (Fusion's sinks into dark backgrounds, and on Windows turns into a navy square): a
// field-coloured box with a soft outline and a tick. Breeze keeps its own checkbox.
CheckBox {
    id: control

    Component.onCompleted: if (!Backend.breezeStyle) indicator = ownIndicator.createObject(control)

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
