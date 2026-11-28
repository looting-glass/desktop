import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A small overlay card in a look's colours (`theme::Look`), so Settings → Appearance shows a
// theme before it's saved: the item name, chips, a selected filter row, price changes and
// marked map mods. A look without a palette (the System theme) is drawn in the desktop's.
Rectangle {
    id: card
    required property var look

    SystemPalette { id: desktop }
    readonly property var p: look.palette
    readonly property color fg: p ? p.windowText : desktop.windowText
    readonly property color accent: p ? p.highlight : desktop.highlight
    readonly property color line: p ? p.mid : desktop.mid

    color: p ? p.window : desktop.window
    border.color: line
    radius: 6
    implicitWidth: 220
    implicitHeight: column.implicitHeight + 20

    component Chip: Rectangle {
        property alias text: chipLabel.text
        property bool accent: false
        implicitWidth: chipLabel.implicitWidth + 14
        implicitHeight: chipLabel.implicitHeight + 6
        radius: 4
        color: "transparent"
        border.color: accent ? card.accent : card.line
        Label { id: chipLabel; anchors.centerIn: parent; font.bold: true; font.pointSize: 8; color: card.fg }
    }

    component MapMod: RowLayout {
        property alias mark: markLabel.text
        property alias markColor: markLabel.color
        property alias text: modLabel.text
        spacing: 6
        Label { id: markLabel; font.bold: true; font.pointSize: 8; Layout.preferredWidth: 62 }
        Label { id: modLabel; color: card.fg; elide: Text.ElideRight; Layout.fillWidth: true }
    }

    ColumnLayout {
        id: column
        anchors { fill: parent; margins: 10 }
        spacing: 6

        Label { text: "Storm Spiral — Ruby Ring"; color: card.accent; font.bold: true; font.pointSize: 11; elide: Text.ElideRight; Layout.fillWidth: true }
        RowLayout {
            spacing: 6
            Chip { text: "Item Level: 79" }
            Chip { text: "3 of 10 filters"; accent: true }
        }
        Item {
            Layout.fillWidth: true
            implicitHeight: lifeRow.implicitHeight + 12
            AccentTint { anchors.fill: parent; accent: card.accent; bar: card.look.selectionBar || card.accent; active: true }
            Label { id: lifeRow; x: 16; anchors.verticalCenter: parent.verticalCenter; text: "+58 total maximum Life"; color: card.fg }
        }
        RowLayout {
            spacing: 12
            Label { text: "▲ 14% this week"; color: card.look.positive }
            Label { text: "▼ 9%"; color: card.look.danger }
        }
        MapMod { mark: "DEADLY"; markColor: card.look.danger; text: "Monsters reflect 18% of Elemental Damage"; Layout.fillWidth: true }
        MapMod { mark: "ANNOYING"; markColor: card.look.caution; text: "Area has patches of Burning Ground"; Layout.fillWidth: true }
    }
}
