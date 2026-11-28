import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts

// A theme to pick on Settings → Appearance: a tiny window in its colours (the selection bar and
// accent on its background) above its name. Without colours of its own it shows the desktop's
// (the System theme), or a pencil (Custom, before any colours are chosen).
AbstractButton {
    id: tile
    required property string label
    // `theme::Swatches` for the system's current mode, or null.
    required property var swatches
    property bool chosen: false
    property bool editable: false

    SystemPalette { id: desktop }
    readonly property color fill: swatches ? swatches.background : editable ? palette.base : desktop.window
    readonly property color accent: swatches ? swatches.accent : desktop.highlight
    readonly property color bar: swatches ? swatches.selectionBar : desktop.highlight
    readonly property color ink: palette.windowText

    implicitWidth: 84
    implicitHeight: column.implicitHeight
    hoverEnabled: true
    background: null

    contentItem: ColumnLayout {
        id: column
        spacing: 4

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 46
            radius: 7
            color: tile.fill
            border.width: tile.chosen ? 2 : 1
            border.color: tile.chosen ? tile.palette.highlight
                : Qt.rgba(tile.ink.r, tile.ink.g, tile.ink.b, tile.hovered ? 0.45 : 0.18)

            Item {
                visible: !tile.editable || tile.swatches
                anchors { fill: parent; margins: 10 }
                Rectangle { width: 3; height: parent.height; radius: 1.5; color: tile.bar }
                Rectangle { x: 9; y: 3; width: parent.width - 18; height: 7; radius: 3.5; color: tile.accent }
                Rectangle {
                    x: 9; y: 16; width: (parent.width - 18) * 0.6; height: 6; radius: 3
                    color: Qt.rgba(tile.accent.r, tile.accent.g, tile.accent.b, 0.45)
                }
            }
            IconImage {
                visible: tile.editable && !tile.swatches
                anchors.centerIn: parent
                source: "qrc:/lootingglass/icons/edit.svg"
                color: tile.ink
                sourceSize: Qt.size(18, 18)
            }
        }
        Label {
            Layout.fillWidth: true
            text: tile.label
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
        }
    }
}
