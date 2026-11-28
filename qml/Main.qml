// Price-check overlay. Made a layer-shell surface (overlay layer) from cpp/main.cpp.
import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Effects
import QtQuick.Layouts
import dev.lootingglass

Window {
    id: win
    objectName: "overlay"
    visible: Backend.overlayVisible
    color: "transparent"
    // Read from C++ to place the window: its monitor, and on Windows its size and position.
    readonly property string overlayScreen: Backend.overlayScreen
    readonly property int overlayWidth: Backend.overlayWidth
    // A height set with the bottom resize edge is fixed (content scrolls); otherwise fit the content.
    readonly property int overlayHeight: Math.min(Backend.overlayMaxHeight > 0 && Backend.snapshotDir === "" ? Backend.overlayMaxHeight : card.implicitHeight,
                                                  Backend.overlayHeightLimit(marginTop, monitorSize))
    property int marginTop: Backend.marginTop
    property int marginSide: Backend.marginSide
    property int side: Backend.side
    // Set from C++ on Linux, where the window covers its monitor and the card is drawn at
    // `cardRect` (the input region). Elsewhere the window is the card, placed by C++.
    property bool coversMonitor: false
    // The monitor's size. When the window covers it, that's the window's size, which comes with
    // the surface; Qt's Screen can name another monitor until the surface has entered its own.
    readonly property size monitorSize: coversMonitor ? Qt.size(width, height) : Qt.size(Screen.width, Screen.height)
    // Where the overlay is on its monitor (read from C++ on Windows), and where the card is
    // drawn in the window.
    readonly property int cardX: Backend.overlayLeftEdge(side, marginSide, overlayWidth, monitorSize)
    readonly property rect cardRect: coversMonitor ? Qt.rect(cardX, marginTop, overlayWidth, overlayHeight)
                                                   : Qt.rect(0, 0, overlayWidth, overlayHeight)
    OverlayDrag { id: overlayDrag; window: win; handle: mover }

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

    // Heading for a filter's group in the list: item properties, pseudo totals, then the item's own mods.
    function filterSection(tag) {
        if (tag === "pseudo") return "Totals (pseudo stats)"
        if (tag === "property" || tag === "weapon" || tag === "socket" || tag === "influence") return "Properties"
        return "Mods"
    }

    // Filter groups the user folded away, by heading; kept for the rest of the session.
    property var collapsed: ({})
    function toggleSection(section) {
        const c = Object.assign({}, collapsed)
        c[section] = !c[section]
        collapsed = c
    }
    // "2 of 6" ticked filters in a group, shown next to its heading.
    function sectionCount(section) {
        rev
        if (!hasSpec) return ""
        const fs = spec.filters.filter(f => filterSection(f.tag) === section)
        return fs.filter(f => f.enabled).length + " of " + fs.length
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
            else if (Backend.snapshotDir === "" && !Backend.pinned) Backend.hideOverlay()
        }
    }
    onVisibleChanged: if (!visible) wasActive = false

    // Colours come from the window's palette, the same one its controls inherit, so the card and
    // the controls always agree (SystemPalette can differ from the Quick Controls palette).
    readonly property var pal: win.palette
    // Bundled icons are tinted with the text colour; the desktop's (theme.desktopIcons) bring their own.
    readonly property var iconTint: theme.desktopIcons ? undefined : pal.windowText
    readonly property color accent: pal.highlight
    AppTheme { id: theme; target: win.palette }
    // Pinned: only Esc (or ✕) closes, so Space and clicking the game leave it open.
    Shortcut { sequences: Backend.pinned ? ["Escape"] : ["Escape", "Space"]; onActivated: Backend.hideOverlay() }

    // Window-style resize edge, dragged like the handle (OverlayDrag).
    component Edge: MouseArea {
        property bool horiz: false
        property bool vert: false
        z: 20
        hoverEnabled: true
        // The card scrolls (a Flickable), which would take over a drag that goes up or down.
        preventStealing: true
        // Resize for the cursor moved (dx, dy) since the press. On the right-hand side the free
        // edge is the left one: the margin keeps the right edge in place as the overlay grows.
        function dragTo(dx, dy) {
            const s = overlayDrag.start
            if (horiz) Backend.overlayWidth = Backend.draggedWidth(s.x, s.width, dx, win.monitorSize)
            if (vert) Backend.overlayMaxHeight = Backend.draggedHeight(s.y, s.height, dy, win.monitorSize)
        }
        onPressed: (m) => overlayDrag.begin(this, m)
        onPositionChanged: (m) => overlayDrag.follow(this, m)
        onReleased: overlayDrag.end(this)
        onCanceled: overlayDrag.end(this)
        // Double-click: back to fitting the content height.
        onDoubleClicked: if (vert) { Backend.overlayMaxHeight = 0; Backend.saveOverlay() }
        Rectangle {
            anchors.fill: parent
            radius: 3
            color: win.accent
            opacity: parent.pressed ? 0.8 : parent.containsMouse ? 0.45 : 0
        }
    }

    // Opens a page in the browser: text first, then a small "opens externally" icon with a tight gap.
    component LinkButton: Button {
        id: linkBtn
        property string url
        // Not `text`: the Breeze style would paint that itself, on top of the label below.
        property string label
        focusPolicy: Qt.NoFocus
        // The Breeze style doesn't pad a custom contentItem; add normal button padding.
        leftPadding: 12
        rightPadding: 10
        enabled: url !== ""
        onClicked: Qt.openUrlExternally(url)
        contentItem: RowLayout {
            spacing: 4
            opacity: linkBtn.enabled ? 1 : 0.5
            Label { text: linkBtn.label; color: pal.buttonText }
            IconImage {
                name: theme.desktopIcons ? "external-link-symbolic" : ""
                source: "qrc:/lootingglass/icons/external-link.svg"
                color: pal.buttonText
                sourceSize: Qt.size(14, 14)
                Layout.preferredWidth: 14
                Layout.preferredHeight: 14
            }
        }
    }

    component Chip: Rectangle {
        property alias text: lbl.text
        property bool accent: false
        property bool warn: false
        property bool muted: false
        implicitWidth: lbl.implicitWidth + 14
        implicitHeight: lbl.implicitHeight + 6
        radius: 4
        color: "transparent"
        border.color: warn ? theme.danger : accent ? pal.highlight : pal.mid
        Label { id: lbl; anchors.centerIn: parent; font.bold: true; font.pointSize: 8; color: warn ? theme.danger : muted ? theme.secondaryText : pal.windowText }
    }

    Rectangle {
        id: card
        x: win.cardRect.x
        y: win.cardRect.y
        width: win.cardRect.width
        height: win.cardRect.height
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
                        // Drag handle (OverlayDrag). Arrow keys move the overlay too, Shift for
                        // bigger steps, for anyone who can't drag precisely.
                        id: mover
                        Layout.preferredWidth: 22
                        Layout.fillHeight: true
                        cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                        hoverEnabled: true
                        // As for the resize edges: the card's scrolling would take over the drag.
                        preventStealing: true
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Grip
                        Accessible.name: "Move the overlay"
                        Accessible.description: "Drag, or use the arrow keys"
                        function dragTo(dx, dy) {
                            const s = overlayDrag.start
                            Backend.placeOverlay(Qt.point(s.x + dx, s.y + dy), win.monitorSize)
                        }
                        onPressed: (m) => {
                            forceActiveFocus()
                            overlayDrag.begin(this, m)
                        }
                        onPositionChanged: (m) => overlayDrag.follow(this, m)
                        onReleased: overlayDrag.end(this)
                        onCanceled: overlayDrag.end(this)
                        // How far an arrow key moves the overlay, or null for other keys.
                        function arrowStep(e) {
                            const step = e.modifiers & Qt.ShiftModifier ? 50 : 10
                            switch (e.key) {
                            case Qt.Key_Left: return Qt.point(-step, 0)
                            case Qt.Key_Right: return Qt.point(step, 0)
                            case Qt.Key_Up: return Qt.point(0, -step)
                            case Qt.Key_Down: return Qt.point(0, step)
                            }
                            return null
                        }
                        Keys.onPressed: (e) => {
                            const d = arrowStep(e)
                            if (d === null) return
                            Backend.placeOverlay(Qt.point(win.cardX + d.x, win.marginTop + d.y), win.monitorSize)
                            e.accepted = true
                        }
                        // Saved once the key is let go, not on every auto-repeat.
                        Keys.onReleased: (e) => {
                            if (arrowStep(e) !== null && !e.isAutoRepeat) Backend.saveOverlay()
                        }
                        Label {
                            anchors.centerIn: parent
                            text: "⠿"
                            font.pixelSize: 18
                            color: mover.containsMouse || mover.pressed || mover.activeFocus ? win.accent : theme.faintText
                        }
                    }
                    Label {
                        text: win.hasSpec ? (win.spec.class || "Item") : mapView.visible ? mapView.m.class : "Looting Glass"
                        font.bold: true
                        font.capitalization: Font.AllUppercase
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                    ToolButton {
                        id: pinBtn
                        icon.name: theme.desktopIcons ? "window-pin" : ""
                        icon.color: win.iconTint
                        icon.source: "qrc:/lootingglass/icons/pin.svg"
                        // The style draws the checked (pinned) state as an active button.
                        checkable: true
                        checked: Backend.pinned
                        focusPolicy: Qt.NoFocus
                        onToggled: {
                            Backend.pinned = checked
                            Backend.saveOverlaySize()
                        }
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: Backend.pinned ? "Pinned: close with ✕ or Esc" : "Keep open when clicking the game"
                        // Accent bar under the pin while it's active.
                        Rectangle {
                            visible: Backend.pinned
                            anchors { bottom: parent.bottom; horizontalCenter: parent.horizontalCenter; bottomMargin: 1 }
                            width: parent.width - 10
                            height: 2
                            radius: 1
                            color: win.accent
                        }
                    }
                    ToolButton { id: closeBtn; icon.name: theme.desktopIcons ? "window-close" : ""; icon.color: win.iconTint; icon.source: "qrc:/lootingglass/icons/close.svg"; focusPolicy: Qt.NoFocus; onClicked: Backend.hideOverlay() }
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
                            Label { text: "poe.ninja"; font.pointSize: 8; font.bold: true; color: theme.secondaryText }
                            Label { text: Backend.ninja; font.pointSize: 12; font.bold: true; Layout.fillWidth: true; elide: Text.ElideRight }
                        }
                        Label {
                            text: Backend.ninjaDetail
                            color: Backend.ninjaDetail.startsWith("▼") ? theme.danger : theme.positive
                        }
                    }
                }

                // Trade site bulk exchange for stackables: the cheapest current offers.
                Rectangle {
                    id: exBox
                    readonly property var ex: {
                        try { return Backend.exchange ? JSON.parse(Backend.exchange) : null } catch (e) { return null }
                    }
                    visible: ex !== null
                    Layout.fillWidth: true
                    implicitHeight: exCol.implicitHeight + 12
                    radius: 6
                    color: "transparent"
                    border.color: Qt.rgba(pal.windowText.r, pal.windowText.g, pal.windowText.b, 0.25)
                    ColumnLayout {
                        id: exCol
                        anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; margins: 8 }
                        spacing: 2
                        RowLayout {
                            Label { text: "Bulk exchange"; font.pointSize: 8; font.bold: true; color: theme.secondaryText; Layout.fillWidth: true }
                            LinkButton { label: "Exchange"; url: exBox.ex ? exBox.ex.url : "" }
                        }
                        Label {
                            text: exBox.ex ? exBox.ex.best + " each" : ""
                            font.pointSize: 12; font.bold: true
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        Repeater {
                            model: exBox.ex ? exBox.ex.offers : []
                            RowLayout {
                                required property var modelData
                                Label { text: modelData.ratio; Layout.fillWidth: true; elide: Text.ElideRight }
                                Label { text: "stock " + modelData.stock; color: theme.faintText }
                            }
                        }
                    }
                }

                Flow {
                    visible: win.hasSpec
                    Layout.fillWidth: true
                    spacing: 6
                    Chip { text: Backend.gameLabel }
                    Chip { visible: win.hasSpec && win.spec.item_level != null; text: "Item Level: " + (win.hasSpec ? win.spec.item_level : "") }
                    Chip {
                        readonly property bool corrupted: win.hasSpec && win.spec.corrupted
                        warn: corrupted
                        muted: !corrupted
                        text: corrupted ? "Corrupted" : "Not corrupted"
                    }
                    Chip { accent: true; text: win.enabledStats + " of " + (win.hasSpec ? win.spec.filters.length : 0) + " filters" }
                }
                Label {
                    visible: Backend.overlayState === "error" || Backend.overlayState === "searchError"
                    text: Backend.message
                    color: theme.danger
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }

                // ── Filters ──────────────────────────────────────────
                // Map check (PoE 1, shown above a map's price check): its mods coloured by the
                // player's marks. Clicking a mod
                // cycles it neutral → annoying → deadly → neutral; marks are stored in `[map_check]`.
                ColumnLayout {
                    id: mapView
                    readonly property var m: {
                        try { return Backend.mapInfo ? JSON.parse(Backend.mapInfo) : null } catch (e) { return null }
                    }
                    function levelColor(level) {
                        return level === "deadly" ? theme.danger : level === "annoying" ? theme.caution : pal.windowText
                    }
                    visible: m !== null && Backend.overlayState !== "error"
                    Layout.fillWidth: true
                    spacing: 6

                    Flow {
                        Layout.fillWidth: true
                        spacing: 6
                        Chip { visible: mapView.m && mapView.m.tier != null; text: "Tier " + (mapView.m ? mapView.m.tier : "") }
                        Repeater {
                            model: mapView.m ? mapView.m.bonuses : []
                            Chip { required property var modelData; text: modelData[0] + " +" + modelData[1] + "%" }
                        }
                        Chip { visible: mapView.m && mapView.m.corrupted; text: "Corrupted"; warn: true }
                    }
                    Label {
                        readonly property int deadly: mapView.m ? mapView.m.deadly : 0
                        readonly property int annoying: mapView.m ? mapView.m.annoying : 0
                        text: deadly > 0 ? deadly + (deadly === 1 ? " deadly mod" : " deadly mods") + (annoying > 0 ? " · " + annoying + " annoying" : "")
                            : annoying > 0 ? annoying + (annoying === 1 ? " annoying mod" : " annoying mods")
                            : "No marked mods"
                        font.bold: deadly + annoying > 0
                        color: deadly > 0 ? theme.danger : annoying > 0 ? theme.caution : theme.faintText
                    }
                    Repeater {
                        model: mapView.m ? mapView.m.mods : []
                        Rectangle {
                            id: mrow
                            required property var modelData
                            readonly property string level: modelData.level
                            readonly property color tint: mapView.levelColor(level)
                            Layout.fillWidth: true
                            implicitHeight: mrl.implicitHeight + 12
                            radius: 4
                            color: level === "" ? (mmouse.containsMouse ? Qt.rgba(tint.r, tint.g, tint.b, 0.06) : "transparent")
                                                : Qt.rgba(tint.r, tint.g, tint.b, mmouse.containsMouse ? 0.24 : 0.16)
                            border.color: level === "" ? "transparent" : tint
                            RowLayout {
                                id: mrl
                                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter; leftMargin: 8; rightMargin: 8 }
                                Label {
                                    text: mrow.modelData.text
                                    wrapMode: Text.Wrap
                                    Layout.fillWidth: true
                                }
                                Label {
                                    visible: mrow.level !== ""
                                    text: mrow.level === "deadly" ? "Deadly" : "Annoying"
                                    font.pointSize: 8
                                    font.bold: true
                                    font.capitalization: Font.AllUppercase
                                    color: mrow.tint
                                }
                            }
                            MouseArea {
                                id: mmouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: Backend.markMapMod(mrow.modelData.template,
                                    mrow.level === "" ? "annoying" : mrow.level === "annoying" ? "deadly" : "")
                            }
                            ToolTip.visible: mmouse.containsMouse
                            ToolTip.delay: 800
                            ToolTip.text: "Click to mark: annoying → deadly → clear"
                        }
                    }
                    Label {
                        text: "Click a mod to mark it. Marks are kept for your next maps."
                        font.pointSize: 8
                        color: theme.faintText
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                }

                Flow {
                    visible: win.hasSpec && win.expanded
                    Layout.fillWidth: true
                    spacing: 10
                    LgCheckBox {
                        visible: win.hasSpec && win.spec.base != null
                        focusPolicy: Qt.NoFocus
                        // Off = search the whole item category (default for rares).
                        text: {
                            win.rev
                            if (!win.hasSpec) return ""
                            if (win.spec.use_base || !win.spec.category) return "Base: " + win.spec.base
                            return "Any " + win.spec.class + " (tick for " + win.spec.base + " only)"
                        }
                        checked: { win.rev; return win.hasSpec && win.spec.use_base }
                        onToggled: { win.spec.use_base = checked; win.edited() }
                    }
                    LgCheckBox {
                        visible: win.hasSpec
                        focusPolicy: Qt.NoFocus
                        text: "Corrupted: " + (win.hasSpec && win.spec.corrupted ? "yes" : "no")
                        checked: win.hasSpec && win.spec.filter_corrupted
                        onToggled: { win.spec.filter_corrupted = checked; win.edited() }
                    }
                }

                Repeater {
                    model: win.hasSpec && win.expanded ? win.spec.filters : []
                    delegate: ColumnLayout {
                        id: fsec
                        required property var modelData
                        required property int index
                        readonly property string section: win.filterSection(modelData.tag)
                        readonly property bool first: index === 0
                            || win.filterSection(win.spec.filters[index - 1].tag) !== section
                        Layout.fillWidth: true
                        spacing: 2

                        readonly property bool folded: win.collapsed[section] === true
                        // Hide folded rows entirely, so they don't leave layout spacing behind.
                        visible: first || !folded

                        // Group heading with a rule, so totals and the mods they sum don't blur
                        // together. Clicking anywhere on it folds the group away.
                        Item {
                            visible: fsec.first
                            Layout.fillWidth: true
                            Layout.topMargin: fsec.index === 0 ? 0 : 6
                            implicitHeight: headRow.implicitHeight
                            RowLayout {
                                id: headRow
                                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                                spacing: 6
                                // Disclosure chevron: › folded, ⌄ unfolded, centred in its square.
                                Item {
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    Rectangle {
                                        anchors.fill: parent
                                        radius: 4
                                        color: pal.windowText
                                        opacity: headMouse.containsMouse ? 0.12 : 0
                                    }
                                    IconImage {
                                        anchors.centerIn: parent
                                        sourceSize: Qt.size(16, 16)
                                        color: pal.windowText
                                        source: fsec.folded ? "qrc:/lootingglass/icons/chevron-right.svg"
                                                            : "qrc:/lootingglass/icons/chevron-down.svg"
                                    }
                                }
                                Label {
                                    text: fsec.section
                                    font.pointSize: 8
                                    font.bold: true
                                    font.capitalization: Font.AllUppercase
                                    color: theme.secondaryText
                                }
                                Rectangle {
                                    Layout.fillWidth: true
                                    implicitHeight: 1
                                    color: pal.windowText
                                    opacity: 0.2
                                }
                                Label {
                                    text: win.sectionCount(fsec.section)
                                    font.pointSize: 8
                                    color: theme.faintText
                                }
                            }
                            MouseArea {
                                id: headMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: win.toggleSection(fsec.section)
                                ToolTip.visible: containsMouse
                                ToolTip.delay: 500
                                ToolTip.text: (fsec.folded ? "Show " : "Hide ") + fsec.section.toLowerCase()
                            }
                        }
                        Rectangle {
                            id: frow
                            visible: !fsec.folded
                            readonly property var modelData: fsec.modelData
                            readonly property int index: fsec.index
                            readonly property bool on: { win.rev; return win.spec.filters[index].enabled }
                            // Yes/no filters (influences, fractured, …) have no value to bound.
                            readonly property bool valued: !(modelData.id.startsWith("flag:") || modelData.tag === "influence")
                            Layout.fillWidth: true
                            implicitHeight: frl.implicitHeight + 8
                            radius: 4
                            color: "transparent"
                            AccentTint { anchors.fill: parent; accent: win.accent; bar: theme.selectionBar; active: frow.on }

                            RowLayout {
                                id: frl
                                // Inset so the min/max fields don't touch the highlighted row's edge.
                                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter
                                          leftMargin: 12; rightMargin: 6 }
                                spacing: 4
                                LgCheckBox {
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
                                        color: frow.on ? pal.windowText : theme.faintText
                                        Layout.fillWidth: true
                                    }
                                    RowLayout {
                                        visible: frow.modelData.tag !== "pseudo"
                                        spacing: 0
                                        Label {
                                            text: frow.modelData.tag
                                            font.pointSize: 7
                                            font.capitalization: Font.AllUppercase
                                            color: theme.faintText
                                        }
                                        // Advanced item text: tier and roll range, top tier highlighted.
                                        Label {
                                            visible: frow.modelData.tier != null
                                            text: " · T" + frow.modelData.tier
                                            font.pointSize: 7
                                            font.bold: frow.modelData.tier === 1
                                            color: frow.modelData.tier === 1 ? win.accent : theme.faintText
                                        }
                                        Label {
                                            visible: frow.modelData.range != null
                                            text: " · " + frow.modelData.range
                                            font.pointSize: 7
                                            color: theme.faintText
                                        }
                                    }
                                }
                                TextField {
                                    Layout.preferredWidth: 54
                                    placeholderText: "min"
                                    // Keep the column aligned, but show nothing for yes/no filters.
                                    opacity: frow.valued ? 1 : 0
                                    enabled: frow.valued
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
                                    // Keep the column aligned, but show nothing for yes/no filters.
                                    opacity: frow.valued ? 1 : 0
                                    enabled: frow.valued
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
                }
                Button {
                    visible: win.hasSpec && win.spec.filters.length > 0
                    flat: true
                    focusPolicy: Qt.NoFocus
                    text: win.expanded ? "Collapse" : "Show filters"
                    icon.name: !theme.desktopIcons ? "" : win.expanded ? "arrow-up" : "arrow-down"
                    icon.color: win.iconTint
                    icon.source: win.expanded ? "qrc:/lootingglass/icons/arrow-up.svg" : "qrc:/lootingglass/icons/arrow-down.svg"
                    Layout.alignment: Qt.AlignHCenter
                    onClicked: win.expanded = !win.expanded
                }

                // ── Results ──────────────────────────────────────────
                RowLayout {
                    visible: win.hasSpec
                    Layout.fillWidth: true
                    Label {
                        text: Backend.overlayState === "loading"
                              ? (Backend.rateWait > 0 ? "Rate limit: waiting " + Backend.rateWait + " s…" : "Searching…")
                              : (Backend.overlayState === "result" ? Backend.message : "")
                        color: theme.secondaryText
                    }
                    ToolButton {
                        focusPolicy: Qt.NoFocus
                        icon.name: theme.desktopIcons ? "view-refresh" : ""
                        icon.color: win.iconTint
                        icon.source: "qrc:/lootingglass/icons/refresh.svg"
                        text: win.hasSpec && win.spec.online ? "Online" : "Any"
                        display: AbstractButton.TextBesideIcon
                        onClicked: { win.spec.online = !win.spec.online; win.edited() }
                    }
                    Item { Layout.fillWidth: true }
                    BusyIndicator { visible: Backend.overlayState === "loading"; running: visible; implicitWidth: 24; implicitHeight: 24 }
                    LinkButton {
                        visible: Backend.infoUrl !== ""
                        label: Backend.infoLabel
                        url: Backend.infoUrl
                    }
                    LinkButton {
                        label: "Trade"
                        url: Backend.tradeUrl
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
                        // Zebra stripes from the text colour: alternateBase is unusable on Windows.
                        color: index % 2 ? Qt.rgba(pal.windowText.r, pal.windowText.g, pal.windowText.b, 0.05) : "transparent"
                        RowLayout {
                            id: rl
                            anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                            Label { text: modelData.price; font.bold: true; Layout.preferredWidth: 120; elide: Text.ElideRight }
                            Label { text: modelData.ilvl ? modelData.ilvl : "—"; Layout.preferredWidth: 36 }
                            Label { text: modelData.age; color: theme.secondaryText; Layout.preferredWidth: 56 }
                            Label {
                                text: modelData.seller
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                color: modelData.corrupted ? theme.danger : theme.faintText
                                // Snapshot mode: blur other players' account names until no letter can be
                                // made out (a lighter blur still showed some for README screenshots).
                                layer.enabled: Backend.snapshotDir !== ""
                                layer.effect: MultiEffect { blurEnabled: true; blurMax: 40; blur: 1.0 }
                            }
                        }
                    }
                }
                Label {
                    visible: Backend.overlayState === "result" && win.rows.length === 0
                    text: "No listings found — try disabling some filters."
                    color: theme.secondaryText
                }
                // Version footer: makes "which version is this?" answerable from any screenshot.
                // A newer release shows up next to it; clicking it opens Settings → About.
                RowLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    Layout.topMargin: 2
                    // Close to the trade site's rate limit: say so before checks start waiting.
                    // Pill in the caution colour (the map check's "annoying"); text in the normal
                    // text colour so it stays readable in light mode too.
                    Rectangle {
                        id: rateWarn
                        visible: Backend.rateWarning !== ""
                        radius: height / 2
                        color: Qt.rgba(theme.caution.r, theme.caution.g, theme.caution.b, 0.18)
                        border.color: theme.caution
                        implicitHeight: warnRow.implicitHeight + 4
                        implicitWidth: warnRow.implicitWidth + 16
                        Layout.maximumWidth: implicitWidth
                        Layout.fillWidth: true
                        Layout.rightMargin: 8
                        RowLayout {
                            id: warnRow
                            spacing: 4
                            anchors { fill: parent; leftMargin: 8; rightMargin: 8 }
                            Label { text: "⚠"; font.bold: true; font.pointSize: 9; color: theme.caution }
                            Label {
                                text: Backend.rateWarning
                                font.bold: true
                                font.pointSize: 8.5
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }
                    }
                    Item { visible: Backend.rateWarning === ""; Layout.fillWidth: true }
                    Label {
                        text: "Looting Glass " + Backend.version
                        font.pointSize: 8.5
                        color: theme.faintText
                    }
                    Label {
                        visible: Backend.updateVersion !== ""
                        text: " · " + Backend.updateVersion + " available"
                        font.pointSize: 8.5
                        font.underline: updateHover.hovered
                        color: win.accent
                        HoverHandler { id: updateHover; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: settings.showAbout() }
                    }
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

    ManualCheck {
        id: manual
        transientParent: null
    }

    // Dev aid (LOOTINGGLASS_SNAPSHOT): render the overlay, then the settings window, to PNGs with
    // transparency — no desktop behind them.
    Timer {
        id: snapTimer
        interval: 1000
        onTriggered: card.grabToImage(r => {
            r.saveToFile(Backend.snapshotDir + (Backend.mapInfo !== "" ? "/map-check.png" : "/overlay.png"))
            console.warn("snapshot: overlay saved")
            // Palette in use, to diagnose colours on other platforms from the snapshot log.
            console.warn("snapshot: palette", ["window", "windowText", "base", "alternateBase", "text",
                "button", "buttonText", "highlight", "highlightedText", "mid", "light", "dark",
                "midlight", "shadow", "placeholderText"].map(r => r + "=" + pal[r]).join(" "))
            settings.show()
            settingsSnap.start()
            manual.snapshot(Backend.snapshotDir)
        })
    }
    Timer {
        id: settingsSnap
        interval: 2500
        onTriggered: settings.snapshot(Backend.snapshotDir)
    }
    Connections {
        target: Backend
        function onOverlayStateChanged() {
            if (Backend.snapshotDir !== "" && Backend.overlayState === "result") snapTimer.restart()
        }
    }

    Connections {
        target: Backend
        function onOpenManual() {
            manual.show()
            manual.raise()
            manual.requestActivate()
        }
        function onOpenSettings() { settings.bringToFront() }
        function onOpenAbout() { settings.showAbout() }
        function onOpenUrl(url) { Qt.openUrlExternally(url) }
    }
}
