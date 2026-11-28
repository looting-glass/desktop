import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts

// A colour setting on Settings → Appearance: a pill tinted with the colour itself, holding a dot
// of it, its name, its hex code to type into, and a pencil that opens the colour picker.
Rectangle {
    id: field
    property alias label: name.text
    property alias text: hex.text
    // Typing finished a whole colour.
    signal edited()
    signal pick()

    readonly property bool valid: hex.acceptableInput
    readonly property color value: valid ? "#" + hex.text.replace("#", "") : palette.mid
    readonly property color ink: palette.text

    implicitWidth: 220
    implicitHeight: 32
    radius: height / 2
    color: Qt.tint(palette.base, Qt.rgba(value.r, value.g, value.b, 0.22))
    // Leans towards the text colour so the pill keeps an edge when the colour is close to the
    // background behind it (the Background colour, typically).
    border.color: Qt.tint(value, Qt.rgba(ink.r, ink.g, ink.b, hex.activeFocus ? 0.1 : 0.3))
    border.width: hex.activeFocus ? 2 : 1

    RowLayout {
        anchors { fill: parent; leftMargin: 5; rightMargin: 3 }
        spacing: 8

        Rectangle {
            implicitWidth: 22
            implicitHeight: 22
            radius: 11
            color: field.value
            border.color: Qt.rgba(field.ink.r, field.ink.g, field.ink.b, 0.25)
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: field.pick()
            }
        }
        Label {
            id: name
            Layout.fillWidth: true
            color: field.ink
            elide: Text.ElideRight
        }
        TextInput {
            id: hex
            Layout.preferredWidth: contentWidth > 0 ? contentWidth : 60
            color: field.ink
            selectionColor: field.palette.highlight
            selectedTextColor: field.palette.highlightedText
            font.family: "monospace"
            selectByMouse: true
            validator: RegularExpressionValidator { regularExpression: /#?[0-9a-fA-F]{6}/ }
            onTextEdited: if (acceptableInput) field.edited()
        }
        // Drawn here rather than a ToolButton, whose square hover box would stick out of the pill.
        AbstractButton {
            id: pencil
            implicitWidth: 26
            implicitHeight: 26
            focusPolicy: Qt.NoFocus
            hoverEnabled: true
            ToolTip.visible: hovered
            ToolTip.text: "Choose a colour"
            onClicked: field.pick()
            background: Rectangle {
                radius: width / 2
                color: Qt.rgba(field.ink.r, field.ink.g, field.ink.b, pencil.pressed ? 0.22 : pencil.hovered ? 0.12 : 0)
            }
            contentItem: Item {
                IconImage {
                    anchors.centerIn: parent
                    source: "qrc:/lootingglass/icons/edit.svg"
                    color: field.ink
                    sourceSize: Qt.size(16, 16)
                }
            }
        }
    }
}
