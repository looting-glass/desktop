import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import dev.lootingglass

// Settings → Appearance: theme tiles, a preset's light or dark variant, the theme code to share
// it, and on the right a preview in the colours chosen, with Custom's colour fields below it.
// Edits stay here until Settings saves them: load() takes the config's [appearance],
// appearance() gives it back.
RowLayout {
    id: page
    spacing: 20

    // The Settings window's colours (AppTheme), for this page's own text.
    required property var appTheme

    // Applies a saved theme that needs a restart (see `theme::Theme::follows_desktop`).
    signal saveAndRestart()

    readonly property var modes: [["system", "Auto"], ["light", "Light"], ["dark", "Dark"]]
    property string themeId: "system"
    property string mode: "system"
    property string colors: ""

    readonly property var preview: JSON.parse(Backend.themePreview(JSON.stringify(appearance())))
    readonly property var look: appTheme.dark ? preview.looks.dark : preview.looks.light
    // Why the theme code can't be used, or empty; Settings won't save until it's fixed.
    readonly property string error: preview.error || ""
    // Why Save and restart couldn't restart, or empty.
    property string restartError: ""
    readonly property bool custom: themeId === "custom"
    readonly property bool preset: !custom && themeId !== "system"

    function appearance() {
        return { theme: themeId, mode: mode, colors: colors }
    }

    function load(a) {
        themeId = a.theme
        mode = a.mode
        colors = a.colors
        showColors()
    }

    function shownCode() {
        const s = look.swatches
        return Backend.themeCode(s.background, s.accent, s.selectionBar)
    }

    // Fills the colour fields from what's shown.
    function showSwatches() {
        background.text = look.swatches.background
        accent.text = look.swatches.accent
        selectionBar.text = look.swatches.selectionBar
    }

    // The colour fields and the theme code, after anything but typing in them changed the theme.
    function showColors() {
        showSwatches()
        code.text = custom ? colors : shownCode()
    }

    function chooseTheme(id) {
        // Custom starts from the colours shown, unless it has working colours of its own.
        const own = JSON.parse(Backend.themePreview(JSON.stringify({ theme: "custom", mode: mode, colors: colors })))
        if (id === "custom" && own.error !== null)
            colors = shownCode()
        themeId = id
        showColors()
    }

    function editColor() {
        const c = Backend.themeCode(background.text, accent.text, selectionBar.text)
        if (c === "")
            return
        colors = c
        code.text = c
    }

    function pickColor(field) {
        picker.target = field
        picker.selectedColor = field.value
        picker.open()
    }

    component SectionLabel: Label {
        font.pointSize: 8
        font.bold: true
        font.capitalization: Font.AllUppercase
        color: page.appTheme.secondaryText
    }

    ColorDialog {
        id: picker
        property var target
        title: "Choose a colour"
        onAccepted: {
            target.text = selectedColor.toString()
            page.editColor()
        }
    }

    ColumnLayout {
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 8
        spacing: 8

        SectionLabel { text: "Theme" }
        GridLayout {
            columns: 3
            columnSpacing: 8
            rowSpacing: 10
            Repeater {
                model: page.preview.choices
                ThemeTile {
                    required property var modelData
                    label: modelData.label
                    swatches: modelData.swatches ? (page.appTheme.dark ? modelData.swatches.dark : modelData.swatches.light) : null
                    editable: modelData.id === "custom"
                    chosen: page.themeId === modelData.id
                    onClicked: page.chooseTheme(modelData.id)
                }
            }
        }

        RowLayout {
            visible: page.preset
            Layout.topMargin: 4
            spacing: 4
            Label { text: "Variant:"; Layout.rightMargin: 4 }
            Repeater {
                model: page.modes
                Button {
                    required property var modelData
                    text: modelData[1]
                    highlighted: page.mode === modelData[0]
                    ToolTip.visible: hovered && modelData[0] === "system"
                    ToolTip.text: "Light or dark, following the system"
                    onClicked: { page.mode = modelData[0]; page.showColors() }
                }
            }
        }

        SectionLabel { text: "Theme code"; visible: themeId !== "system"; Layout.topMargin: 6 }
        RowLayout {
            visible: themeId !== "system"
            spacing: 6
            TextField {
                id: code
                font.family: "monospace"
                selectByMouse: true
                Layout.preferredWidth: 200
                // A pasted or typed code becomes the custom colours.
                onTextEdited: {
                    page.themeId = "custom"
                    page.colors = text.trim()
                    if (page.error === "")
                        page.showSwatches()
                }
            }
            Button {
                text: "Copy"
                onClicked: { code.selectAll(); code.copy(); code.deselect() }
            }
        }
        Label {
            visible: page.error !== ""
            text: page.error
            color: page.look.danger
            wrapMode: Text.Wrap
            Layout.preferredWidth: 268
        }
        Label {
            visible: themeId !== "system" && page.error === ""
            text: "Share the code, or paste someone's here to use their theme."
            color: page.appTheme.secondaryText
            wrapMode: Text.Wrap
            Layout.preferredWidth: 268
        }

        ColumnLayout {
            visible: page.preview.needsRestart
            Layout.topMargin: 6
            spacing: 6
            Label {
                text: themeId === "system"
                    ? "Your desktop's colours take effect after Looting Glass restarts."
                    : "Looting Glass's own themes take effect after it restarts."
                wrapMode: Text.Wrap
                Layout.preferredWidth: 268
            }
            Button { text: "Save and restart"; onClicked: page.saveAndRestart() }
            Label {
                visible: page.restartError !== ""
                text: page.restartError
                color: page.look.danger
                wrapMode: Text.Wrap
                Layout.preferredWidth: 268
            }
        }
    }

    ColumnLayout {
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 8
        spacing: 8

        SectionLabel { text: "Preview" }
        ThemePreview { look: page.look }

        SectionLabel { text: "Colours"; visible: page.custom; Layout.topMargin: 6 }
        ColorField { id: background; label: "Background"; visible: page.custom; onEdited: page.editColor(); onPick: page.pickColor(this) }
        ColorField { id: accent; label: "Accent"; visible: page.custom; onEdited: page.editColor(); onPick: page.pickColor(this) }
        ColorField { id: selectionBar; label: "Selection bar"; visible: page.custom; onEdited: page.editColor(); onPick: page.pickColor(this) }
    }
}
