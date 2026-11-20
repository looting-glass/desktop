// Price-check overlay. Made a layer-shell surface (overlay layer) from cpp/main.cpp.
import QtQuick
import QtQuick.Controls
import QtQuick.Effects
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.lootlens

Window {
    id: win
    objectName: "overlay"
    visible: Backend.overlayVisible
    color: "transparent"
    width: Backend.overlayWidth
    // A height set with the bottom resize edge is fixed (content scrolls); otherwise fit the content.
    height: Math.min(Backend.overlayMaxHeight > 0 && Backend.snapshotDir === "" ? Backend.overlayMaxHeight : card.implicitHeight,
                     Screen.height - marginTop - 40)

    // Read from C++ for layer-shell anchors/margins.
    property int marginTop: Backend.marginTop
    property int marginSide: Backend.marginSide
    property int side: Backend.side

    // Editable copy of the search spec; `rev` bumps on every edit so bindings refresh.
    property var spec: null
    property int rev: 0
    property bool expanded: true
    readonly property bool hasSpec: spec !== null
    readonly property var rows: {
        try { return JSON.parse(Backend.listings || "[]") } catch (e) { return [] }
    }
    readonly property int enabledStats: {
        rev
        return hasSpec ? spec.filters.filter(f => f.enabled).length : 0
    }

    function edited() {
        rev++
        searchTimer.restart()
    }

    Timer {
        id: searchTimer
        interval: 700
        onTriggered: if (win.hasSpec) Backend.search(JSON.stringify(win.spec))
    }
    Connections {
        target: Backend
        function onSpecChanged() {
            searchTimer.stop()
            win.spec = Backend.spec ? JSON.parse(Backend.spec) : null
            win.rev++
        }
    }

    // Close when clicking back into the game. KWin may briefly take focus away right after
    // showing (e.g. on the first resize); treat an early focus loss as noise and re-activate.
    property bool wasActive: false
    property double activeSince: 0
    onActiveChanged: {
        if (active) {
            wasActive = true
            activeSince = Date.now()
        } else if (wasActive && visible) {
            if (Date.now() - activeSince < 600) requestActivate()
            else if (Backend.snapshotDir === "") Backend.hideOverlay()
        }
    }
    onVisibleChanged: if (!visible) wasActive = false

    SystemPalette { id: pal }
    Shortcut { sequences: ["Escape", "Space"]; onActivated: Backend.hideOverlay() }

    // Window-style resize edge. Height (and width when the overlay is on the left) resize live,
    // since the surface origin stays put. Growing a right-anchored overlay moves its origin,
    // which KWin applies asynchronously, so that width is applied on release.
    component Edge: MouseArea {
        property bool horiz: false
        property bool vert: false
        property real px: 0
        property real py: 0
        property real startW: 0
        property real startH: 0
        z: 20
        hoverEnabled: true
        function clampW(w) { return Math.round(Math.max(300, Math.min(w, Screen.width))) }
        function clampH(h) { return Math.round(Math.max(150, Math.min(h, Screen.height - win.marginTop - 40))) }
        onPressed: (m) => {
            const p = mapToItem(null, m.x, m.y)
            px = p.x; py = p.y; startW = win.width; startH = win.height
        }
        onPositionChanged: (m) => {
            if (!pressed) return
            const p = mapToItem(null, m.x, m.y)
            if (vert) Backend.overlayMaxHeight = clampH(startH + p.y - py)
            if (horiz && win.side === 0) Backend.overlayWidth = clampW(startW + p.x - px)
        }
        onReleased: (m) => {
            const p = mapToItem(null, m.x, m.y)
            if (horiz && win.side === 1) Backend.overlayWidth = clampW(startW + px - p.x)
            Backend.saveOverlaySize()
        }
        // Double-click: back to fitting the content height.
        onDoubleClicked: if (vert) { Backend.overlayMaxHeight = 0; Backend.saveOverlaySize() }
        Rectangle {
            anchors.fill: parent
            radius: 3
            color: pal.highlight
            opacity: parent.pressed ? 0.8 : parent.containsMouse ? 0.45 : 0
        }
    }

    component Chip: Rectangle {
        property alias text: lbl.text
        property bool accent: false
        implicitWidth: lbl.implicitWidth + 14
        implicitHeight: lbl.implicitHeight + 6
        radius: 4
        color: "transparent"
        border.color: accent ? pal.highlight : pal.mid
        Label { id: lbl; anchors.centerIn: parent; font.bold: true; font.pointSize: 8 }
    }

    Rectangle {
        id: card
        anchors.fill: parent
        implicitHeight: col.implicitHeight + 24
        radius: 8
        color: pal.window
        border.color: pal.mid

        ScrollView {
            anchors { fill: parent; margins: 12 }
            contentWidth: availableWidth

            ColumnLayout {
                id: col
                width: parent.width
                spacing: 6

                // ── Header (drag to move the overlay) ────────────────
                RowLayout {
                    Layout.fillWidth: true
                    MouseArea {
                        // Moving a layer-shell surface = changing its margins. Applied once on release:
                        // the compositor moves the surface asynchronously, so live updates feed back.
                        id: mover
                        Layout.preferredWidth: 22
                        Layout.fillHeight: true
                        cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                        hoverEnabled: true
                        property real px: 0
                        property real py: 0
                        onPressed: (m) => { const p = mapToItem(null, m.x, m.y); px = p.x; py = p.y }
                        onReleased: (m) => {
                            const p = mapToItem(null, m.x, m.y)
                            const dx = p.x - px
                            const dy = p.y - py
                            const sideM = Backend.marginSide + (win.side === 1 ? -dx : dx)
                            Backend.marginSide = Math.round(Math.max(0, Math.min(sideM, Screen.width - win.width)))
                            Backend.marginTop = Math.round(Math.max(0, Math.min(Backend.marginTop + dy, Screen.height - 120)))
                            Backend.saveOverlaySize()
                        }
                        Label {
                            anchors.centerIn: parent
                            text: "⠿"
                            font.pixelSize: 18
                            color: mover.containsMouse || mover.pressed ? pal.highlight : pal.windowText
                            opacity: mover.containsMouse || mover.pressed ? 1 : 0.5
                        }
                    }
                    Label {
                        text: win.hasSpec ? (win.spec.class || "Item") : "Loot Lens"
                        font.bold: true
                        font.capitalization: Font.AllUppercase
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                    Label {
                        visible: win.hasSpec
                        text: win.hasSpec && win.spec.corrupted ? "Corrupted" : "Not corrupted"
                        color: win.hasSpec && win.spec.corrupted ? "#e06c6c" : pal.windowText
                        opacity: win.hasSpec && win.spec.corrupted ? 1 : 0.55
                    }
                    ToolButton { icon.name: "window-close"; focusPolicy: Qt.NoFocus; onClicked: Backend.hideOverlay() }
                }
                Label {
                    visible: Backend.overlayState !== "error"
                    text: Backend.title
                    font.pointSize: 11
                    font.bold: true
                    color: pal.highlight
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                // poe.ninja price for stackables
                Rectangle {
                    visible: Backend.ninja !== ""
                    Layout.fillWidth: true
                    implicitHeight: ninjaCol.implicitHeight + 12
                    radius: 6
                    color: Qt.rgba(pal.highlight.r, pal.highlight.g, pal.highlight.b, 0.18)
                    border.color: pal.highlight
                    ColumnLayout {
                        id: ninjaCol
                        anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; margins: 8 }
                        spacing: 2
                        RowLayout {
                            Label { text: "poe.ninja"; font.pointSize: 8; font.bold: true; opacity: 0.7 }
                            Label { text: Backend.ninja; font.pointSize: 12; font.bold: true; Layout.fillWidth: true; elide: Text.ElideRight }
                        }
                        Label {
                            text: Backend.ninjaDetail
                            opacity: 0.75
                            color: Backend.ninjaDetail.startsWith("▼") ? "#e06c6c" : "#6cc070"
                        }
                    }
                }

                Flow {
                    visible: win.hasSpec
                    Layout.fillWidth: true
                    spacing: 6
                    Chip { text: Backend.gameLabel }
                    Chip { visible: win.hasSpec && win.spec.item_level != null; text: "Item Level: " + (win.hasSpec ? win.spec.item_level : "") }
                    Chip { accent: true; text: win.enabledStats + " of " + (win.hasSpec ? win.spec.filters.length : 0) + " filters" }
                }
                Label {
                    visible: Backend.overlayState === "error" || Backend.overlayState === "searchError"
                    text: Backend.message
                    color: "#e06c6c"
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }

                // ── Filters ──────────────────────────────────────────
                Flow {
                    visible: win.hasSpec && win.expanded
                    Layout.fillWidth: true
                    spacing: 10
                    CheckBox {
                        visible: win.hasSpec && win.spec.base != null
                        focusPolicy: Qt.NoFocus
                        text: "Base: " + (win.hasSpec ? win.spec.base : "")
                        checked: win.hasSpec && win.spec.use_base
                        onToggled: { win.spec.use_base = checked; win.edited() }
                    }
                    CheckBox {
                        visible: win.hasSpec
                        focusPolicy: Qt.NoFocus
                        text: "Corrupted: " + (win.hasSpec && win.spec.corrupted ? "yes" : "no")
                        checked: win.hasSpec && win.spec.filter_corrupted
                        onToggled: { win.spec.filter_corrupted = checked; win.edited() }
                    }
                }

                Repeater {
                    model: win.hasSpec && win.expanded ? win.spec.filters : []
                    delegate: Rectangle {
                        id: frow
                        required property var modelData
                        required property int index
                        readonly property bool on: { win.rev; return win.spec.filters[index].enabled }
                        Layout.fillWidth: true
                        implicitHeight: frl.implicitHeight + 8
                        radius: 4
                        color: on ? Qt.rgba(pal.highlight.r, pal.highlight.g, pal.highlight.b, 0.12) : "transparent"

                        RowLayout {
                            id: frl
                            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                            spacing: 4
                            CheckBox {
                                focusPolicy: Qt.NoFocus
                                checked: frow.modelData.enabled
                                onToggled: { win.spec.filters[frow.index].enabled = checked; win.edited() }
                            }
                            ColumnLayout {
                                spacing: 0
                                Layout.fillWidth: true
                                Label {
                                    text: frow.modelData.label
                                    wrapMode: Text.Wrap
                                    opacity: frow.on ? 1 : 0.6
                                    Layout.fillWidth: true
                                }
                                Label {
                                    text: frow.modelData.tag
                                    font.pointSize: 7
                                    font.capitalization: Font.AllUppercase
                                    opacity: 0.5
                                }
                            }
                            TextField {
                                Layout.preferredWidth: 54
                                placeholderText: "min"
                                horizontalAlignment: Text.AlignHCenter
                                text: frow.modelData.min != null ? frow.modelData.min : ""
                                validator: DoubleValidator {}
                                onEditingFinished: {
                                    win.spec.filters[frow.index].min = text === "" ? null : Number(text)
                                    win.edited()
                                }
                            }
                            TextField {
                                Layout.preferredWidth: 54
                                placeholderText: "max"
                                horizontalAlignment: Text.AlignHCenter
                                text: frow.modelData.max != null ? frow.modelData.max : ""
                                validator: DoubleValidator {}
                                onEditingFinished: {
                                    win.spec.filters[frow.index].max = text === "" ? null : Number(text)
                                    win.edited()
                                }
                            }
                        }
                    }
                }
                Button {
                    visible: win.hasSpec && win.spec.filters.length > 0
                    flat: true
                    focusPolicy: Qt.NoFocus
                    text: win.expanded ? "Collapse" : "Show filters"
                    icon.name: win.expanded ? "arrow-up" : "arrow-down"
                    Layout.alignment: Qt.AlignHCenter
                    onClicked: win.expanded = !win.expanded
                }

                // ── Results ──────────────────────────────────────────
                RowLayout {
                    visible: win.hasSpec
                    Layout.fillWidth: true
                    Label {
                        text: Backend.overlayState === "loading" ? "Searching…" : (Backend.overlayState === "result" ? Backend.message : "")
                        opacity: 0.8
                    }
                    ToolButton {
                        focusPolicy: Qt.NoFocus
                        icon.name: "view-refresh"
                        text: win.hasSpec && win.spec.online ? "Online" : "Any"
                        display: AbstractButton.TextBesideIcon
                        onClicked: { win.spec.online = !win.spec.online; win.edited() }
                    }
                    Item { Layout.fillWidth: true }
                    BusyIndicator { visible: Backend.overlayState === "loading"; running: visible; implicitWidth: 24; implicitHeight: 24 }
                    Button {
                        id: tradeBtn
                        focusPolicy: Qt.NoFocus
                        // The Breeze style doesn't pad a custom contentItem; add normal button padding.
                        leftPadding: 12
                        rightPadding: 10
                        enabled: Backend.tradeUrl !== ""
                        onClicked: Qt.openUrlExternally(Backend.tradeUrl)
                        // Text first, then a small "opens externally" icon with a tight gap.
                        contentItem: RowLayout {
                            spacing: 4
                            opacity: tradeBtn.enabled ? 1 : 0.5
                            Label { text: "Trade"; color: pal.buttonText }
                            Kirigami.Icon {
                                source: "external-link-symbolic"
                                color: pal.buttonText
                                isMask: true
                                Layout.preferredWidth: 14
                                Layout.preferredHeight: 14
                            }
                        }
                    }
                }

                RowLayout {
                    visible: win.rows.length > 0
                    Layout.fillWidth: true
                    Label { text: "Price"; font.bold: true; Layout.preferredWidth: 120 }
                    Label { text: "iLvl"; font.bold: true; Layout.preferredWidth: 36 }
                    Label { text: "Listed"; font.bold: true; Layout.preferredWidth: 56 }
                    Label { text: "Seller"; font.bold: true; Layout.fillWidth: true }
                }
                Repeater {
                    model: win.rows
                    delegate: Rectangle {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        implicitHeight: rl.implicitHeight + 6
                        color: index % 2 ? Qt.rgba(pal.alternateBase.r, pal.alternateBase.g, pal.alternateBase.b, 0.6) : "transparent"
                        RowLayout {
                            id: rl
                            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                            Label { text: modelData.price; font.bold: true; Layout.preferredWidth: 120; elide: Text.ElideRight }
                            Label { text: modelData.ilvl ? modelData.ilvl : "—"; Layout.preferredWidth: 36 }
                            Label { text: modelData.age; opacity: 0.8; Layout.preferredWidth: 56 }
                            Label {
                                text: modelData.seller
                                opacity: 0.6
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                color: modelData.corrupted ? "#e06c6c" : pal.windowText
                                // Snapshot mode: blur other players' account names.
                                layer.enabled: Backend.snapshotDir !== ""
                                layer.effect: MultiEffect { blurEnabled: true; blurMax: 24; blur: 1.0 }
                            }
                        }
                    }
                }
                Label {
                    visible: Backend.overlayState === "result" && win.rows.length === 0
                    text: "No listings found — try disabling some filters."
                    opacity: 0.7
                }
            }
        }
        // Resize handles on the free edges (the other edges sit against the screen).
        Edge {
            horiz: true
            x: win.side === 1 ? 0 : parent.width - width
            width: 6
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            cursorShape: Qt.SizeHorCursor
        }
        Edge {
            vert: true
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 6
            cursorShape: Qt.SizeVerCursor
        }
        Edge {
            horiz: true
            vert: true
            x: win.side === 1 ? 0 : parent.width - width
            anchors.bottom: parent.bottom
            width: 16
            height: 16
            cursorShape: win.side === 1 ? Qt.SizeBDiagCursor : Qt.SizeFDiagCursor
        }
    }

    Settings {
        id: settings
        transientParent: null
    }

    // Dev aid (LOOTLENS_SNAPSHOT): render the overlay, then the settings window, to PNGs with
    // transparency — no desktop behind them.
    Timer {
        id: snapTimer
        interval: 1000
        onTriggered: card.grabToImage(r => {
            r.saveToFile(Backend.snapshotDir + "/overlay.png")
            console.warn("snapshot: overlay saved")
            settings.show()
            settingsSnap.start()
        })
    }
    Timer {
        id: settingsSnap
        interval: 2500
        onTriggered: settings.snapshot(Backend.snapshotDir + "/settings.png")
    }
    Connections {
        target: Backend
        function onOverlayStateChanged() {
            if (Backend.snapshotDir !== "" && Backend.overlayState === "result") snapTimer.restart()
        }
    }

    Connections {
        target: Backend
        function onOpenSettings() {
            settings.show()
            settings.raise()
            settings.requestActivate()
        }
    }
}
