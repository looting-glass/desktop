import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import dev.lootingglass

// Manual price check: paste item text copied from the game; results open in the overlay.
ApplicationWindow {
    id: win
    title: "Looting Glass"
    width: 560
    height: 460
    minimumWidth: 420
    minimumHeight: 340
    visible: false
    KdeLook { target: win.palette }

    function check() {
        const err = Backend.checkText(input.text)
        error.text = err
    }

    onVisibleChanged: if (visible) { error.text = ""; input.forceActiveFocus() }

    // LOOTINGGLASS_SNAPSHOT: save the window with sample item text as manual-check.png.
    function snapshot(dir) {
        show()
        input.text = Backend.demoText
        snapTimer.dir = dir
        snapTimer.start()
    }
    Timer {
        id: snapTimer
        property string dir
        interval: 800
        onTriggered: page.grabToImage(r => {
            r.saveToFile(dir + "/manual-check.png")
            console.warn("snapshot: manual saved")
        })
    }

    Rectangle {
        id: page
        anchors.fill: parent
        color: win.palette.window

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 8

            RowLayout {
                spacing: 8
                Image {
                    source: "qrc:/lootingglass/icons/lootingglass.svg"
                    sourceSize: Qt.size(40, 40)
                    Layout.preferredWidth: 40
                    Layout.preferredHeight: 40
                }
                ColumnLayout {
                    spacing: 0
                    Label { text: "Manual price check"; font.pointSize: Qt.application.font.pointSize * 1.35 }
                    Label {
                        text: "Checking against " + Backend.gameLabel
                        opacity: 0.7
                    }
                }
                Item { Layout.fillWidth: true }
                ToolButton {
                    icon.name: Qt.platform.os === "linux" ? "configure" : ""
                    icon.color: Qt.platform.os === "linux" ? undefined : win.palette.windowText
                    icon.source: "qrc:/lootingglass/icons/settings.svg"
                    text: "Settings"
                    display: AbstractButton.IconOnly
                    ToolTip.visible: hovered
                    ToolTip.text: "Settings"
                    onClicked: Backend.openSettingsWindow()
                }
            }

            Label {
                text: "Hover an item in game, press Ctrl+C (or Ctrl+Alt+C for full mod details), then paste it below."
                wrapMode: Text.Wrap
                opacity: 0.85
                Layout.fillWidth: true
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                TextArea {
                    id: input
                    placeholderText: "Item Class: Boots\nRarity: Rare\n…"
                    font.family: "monospace"
                    wrapMode: TextEdit.NoWrap
                    selectByMouse: true
                    onTextChanged: error.text = ""
                    Keys.onPressed: (e) => {
                        if ((e.key === Qt.Key_Return || e.key === Qt.Key_Enter) && (e.modifiers & Qt.ControlModifier)) {
                            win.check()
                            e.accepted = true
                        }
                    }
                }
            }

            Label {
                id: error
                visible: text !== ""
                color: "#da4453"
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }

            RowLayout {
                Layout.fillWidth: true
                Button {
                    text: "Paste"
                    icon.name: "edit-paste"
                    onClicked: { input.selectAll(); input.paste() }
                }
                Button {
                    text: "Clear"
                    icon.name: "edit-clear"
                    enabled: input.text !== ""
                    onClicked: input.clear()
                }
                Item { Layout.fillWidth: true }
                Label { text: "Ctrl+Enter"; opacity: 0.5 }
                Button {
                    text: "Check price"
                    icon.source: "qrc:/lootingglass/icons/lootingglass.svg"
                    icon.color: "transparent"
                    highlighted: true
                    enabled: input.text.trim() !== ""
                    onClicked: win.check()
                }
            }
        }
    }
}
