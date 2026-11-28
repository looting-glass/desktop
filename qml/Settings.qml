import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import dev.lootingglass

// Settings window: category sidebar (like KDE System Settings) + one short form per page.
ApplicationWindow {
    id: win
    title: "Looting Glass Settings"
    width: 760
    height: 500
    minimumWidth: 620
    minimumHeight: 420
    visible: false

    property var cfg: ({})
    AppTheme { id: theme; target: win.palette }
    readonly property var statuses: ["online", "any", "securable"]
    // Only present in the config on platforms where Looting Glass registers hotkeys itself (Windows).
    readonly property bool ownShortcuts: cfg.shortcuts !== undefined
    readonly property var shortcutFields: [
        ["price_check", "Price check:"],
        ["toggle_game", "Switch game:"],
        ["manual_check", "Manual check:"],
        ["settings", "Open settings:"],
        ["hide_overlay", "Hide overlay:"],
        ["item_info", "Item info:"],
    ]
    readonly property var infoSites: ["poedb", "wiki"]

    function leaguesFor(key) {
        try { return JSON.parse(Backend.leagues || "{}")[key] || [] } catch (e) { return [] }
    }
    function syncLeague(box, value) {
        const i = box.find(value)
        if (i >= 0) box.currentIndex = i
        else box.editText = value
    }
    function load() {
        cfg = JSON.parse(Backend.configJson())
        game.currentIndex = cfg.game === "poe2" ? 1 : 0
        mapCheckOn.checked = cfg.map_check.enabled
        mapRev++
        syncLeague(league1, cfg.poe1.league)
        syncLeague(league2, cfg.poe2.league)
        infoSite.currentIndex = Math.max(0, infoSites.indexOf(cfg.links.site))
        status.currentIndex = Math.max(0, statuses.indexOf(cfg.trade.status))
        tolerance.value = cfg.trade.stat_tolerance
        minFromTier.checked = cfg.trade.min_from_tier
        results.value = cfg.trade.max_results
        side.currentIndex = cfg.overlay.side === "left" ? 0 : 1
        width_.value = cfg.overlay.width
        top_.value = cfg.overlay.margin_top
        sideMargin.value = cfg.overlay.margin_side
        maxHeight.value = cfg.overlay.max_height
        appearancePage.load(cfg.appearance)
        contact.text = cfg.contact
        checkUpdates.checked = cfg.updates.check
        if (ownShortcuts)
            for (let i = 0; i < shortcutFields.length; i++)
                shortcutRows.itemAt(i).value = cfg.shortcuts[shortcutFields[i][0]]
        saveStatus.text = ""
        Backend.refreshLeagues()
        Backend.refreshShortcuts()
    }
    // Shortcuts page (KDE): recorded keys wait here until Save; Close discards them.
    // id -> { key, keys: "Ctrl+D" (empty = none), conflict: who else uses it }
    property var pendingShortcuts: ({})
    function stageShortcut(id, key, keys, conflict) {
        const p = Object.assign({}, pendingShortcuts)
        p[id] = { key: key, keys: keys, conflict: conflict }
        pendingShortcuts = p
    }
    Connections {
        target: Backend
        function onShortcutCheckChanged() {
            try {
                const c = JSON.parse(Backend.shortcutCheck)
                win.stageShortcut(c.id, c.key, c.keys, c.conflict)
            } catch (e) {}
        }
    }

    // Maps page: marked PoE 1 map mods, edited in `cfg` and saved with Save.
    property int mapRev: 0
    function mapMods() { mapRev; return cfg ? cfg.map_check : null }
    function setMapMark(template, level) {
        const m = mapMods()
        m.deadly = m.deadly.filter(t => t !== template)
        m.annoying = m.annoying.filter(t => t !== template)
        if (level === "deadly" || level === "annoying") m[level].push(template)
        mapRev++
    }

    function save() {
        // An unreadable theme code would be saved and then quietly shown as Looting Glass.
        if (appearancePage.error !== "") {
            nav.currentIndex = visibleCategories.findIndex(cat => cat.name === "Appearance")
            saveStatus.text = "Not saved: the theme code isn't valid."
            return false
        }
        const c = cfg
        c.game = game.currentIndex === 1 ? "poe2" : "poe1"
        c.poe1.league = league1.editText.trim() || "Standard"
        c.poe2.league = league2.editText.trim() || "Standard"
        c.links.site = infoSites[infoSite.currentIndex]
        c.map_check.enabled = mapCheckOn.checked
        c.trade.status = statuses[status.currentIndex]
        c.trade.stat_tolerance = tolerance.value
        c.trade.min_from_tier = minFromTier.checked
        c.trade.max_results = results.value
        c.overlay.side = side.currentIndex === 0 ? "left" : "right"
        c.overlay.width = width_.value
        c.overlay.margin_top = top_.value
        c.overlay.margin_side = sideMargin.value
        c.overlay.max_height = maxHeight.value
        c.appearance = appearancePage.appearance()
        c.contact = contact.text
        c.updates.check = checkUpdates.checked
        if (ownShortcuts)
            for (let i = 0; i < shortcutFields.length; i++)
                c.shortcuts[shortcutFields[i][0]] = shortcutRows.itemAt(i).value.trim()
        for (const id in pendingShortcuts)
            Backend.setShortcut(id, pendingShortcuts[id].key)
        pendingShortcuts = {}
        const err = Backend.saveConfig(JSON.stringify(c))
        saveStatus.text = err === "" ? "Saved." : "Error: " + err
        return err === ""
    }

    onVisibleChanged: {
        if (reopening) return
        if (visible) {
            Backend.shortcutError = ""
            load()
        } else {
            Backend.blockShortcuts(false)
            pendingShortcuts = {}
        }
    }

    // A global shortcut's keys (KDE): click, then press the combination. Global shortcuts are
    // paused meanwhile, so the keys don't also trigger what they're bound to.
    component KeyButton: Button {
        id: kb
        property string shortcutId
        property string keys
        property bool changed: false
        property bool recording: false
        property int mods: 0
        readonly property int modMask: Qt.ControlModifier | Qt.ShiftModifier | Qt.AltModifier | Qt.MetaModifier
        Layout.preferredWidth: 170
        highlighted: recording
        text: recording ? win.modText(mods) + "…" : (keys || "None") + (changed ? " •" : "")
        font.bold: changed && !recording
        onClicked: {
            if (recording) return
            recording = true
            mods = 0
            Backend.shortcutError = ""
            Backend.blockShortcuts(true)
            forceActiveFocus()
        }
        function stop() {
            if (!recording) return
            recording = false
            Backend.blockShortcuts(false)
        }
        onActiveFocusChanged: if (!activeFocus) stop()
        Keys.onPressed: e => {
            if (!recording) return
            e.accepted = true
            const m = e.modifiers & modMask
            if ([Qt.Key_Control, Qt.Key_Shift, Qt.Key_Alt, Qt.Key_Meta, Qt.Key_AltGr, Qt.Key_Super_L, Qt.Key_Super_R].indexOf(e.key) >= 0) {
                mods = m
                return
            }
            if (e.key === Qt.Key_Escape && m === 0) { stop(); return }
            const clear = (e.key === Qt.Key_Backspace || e.key === Qt.Key_Delete) && m === 0
            stop()
            if (clear) win.stageShortcut(shortcutId, 0, "", "")
            else Backend.checkShortcut(shortcutId, m | e.key)
        }
        Keys.onReleased: e => { if (recording) { mods = e.modifiers & modMask; e.accepted = true } }
    }
    function modText(m) {
        const parts = []
        if (m & Qt.MetaModifier) parts.push("Meta")
        if (m & Qt.ControlModifier) parts.push("Ctrl")
        if (m & Qt.AltModifier) parts.push("Alt")
        if (m & Qt.ShiftModifier) parts.push("Shift")
        return parts.length ? parts.join("+") + "+" : "Press keys"
    }

    function showAbout() {
        nav.currentIndex = visibleCategories.length - 1
        bringToFront()
    }

    // Wayland doesn't let an app raise its own window from the tray, so an open window behind
    // others is closed and shown again, keeping unsaved edits (no reload, nothing discarded).
    property bool reopening: false
    function bringToFront() {
        if (visible) {
            reopening = true
            hide()
            show()
            reopening = false
        } else {
            show()
        }
        raise()
        requestActivate()
    }

    Connections {
        target: Backend
        function onLeaguesChanged() {
            win.syncLeague(league1, win.cfg.poe1.league)
            win.syncLeague(league2, win.cfg.poe2.league)
        }
    }

    // Page background as a real item so it can be captured (LOOTINGGLASS_SNAPSHOT):
    // saves the pages in `shots` as settings-<page>.png in `dir`.
    function snapshot(dir) {
        win.width = 760
        win.height = 440
        snapTimer.dir = dir
        snapTimer.step = 0
        nav.currentIndex = 0
        snapTimer.start()
    }
    Timer {
        id: snapTimer
        property string dir
        property int step: 0
        readonly property var shots: [["settings-game.png", 0], ["settings-trade.png", 1],
            ["settings-appearance.png", win.visibleCategories.findIndex(c => c.name === "Appearance")],
            ["settings-shortcuts.png", win.visibleCategories.findIndex(c => c.name === "Shortcuts")],
            ["settings-about.png", win.visibleCategories.length - 1]]
        interval: 800
        onTriggered: page.grabToImage(r => {
            r.saveToFile(dir + "/" + shots[step][0])
            if (step + 1 < shots.length) {
                step++
                nav.currentIndex = shots[step][1]
                start()
            } else {
                console.warn("snapshot: settings saved")
            }
        })
    }

    readonly property var categories: [
        { name: "Game", icon: "nav-game" },
        { name: "Trade", icon: "nav-trade" },
        { name: "Overlay", icon: "nav-overlay" },
        { name: "Appearance", icon: "nav-appearance" },
        { name: "Shortcuts", icon: "nav-shortcuts" },
        { name: "Maps", icon: "nav-maps" },
        { name: "About", icon: "nav-about" },
    ]
    // The map check is PoE 1 only: its page shows while PoE 1 is selected and the check is on.
    readonly property var visibleCategories: categories.filter(c => c.name !== "Maps" || (game.currentIndex === 0 && mapCheckOn.checked))

    Rectangle {
        id: page
        anchors.fill: parent
        color: win.palette.window

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                // ── Sidebar ──────────────────────────────────────────
                Rectangle {
                    Layout.fillHeight: true
                    Layout.preferredWidth: 180
                    color: win.palette.base

                    ListView {
                        id: nav
                        anchors.fill: parent
                        anchors.margins: 4
                        // Items start further right; their highlight reaches back (leftInset) so the
                        // accent bar has room before the icon. Breeze ignores leftPadding here.
                        anchors.leftMargin: 12
                        model: win.visibleCategories
                        currentIndex: 0
                        spacing: 2
                        delegate: ItemDelegate {
                            id: navItem
                            required property var modelData
                            required property int index
                            width: ListView.view.width
                            text: modelData.name
                            // Bundled monochrome icons, tinted with the text colour. Drawn here rather
                            // than via icon.source: Breeze doesn't recolour bundled icons.
                            contentItem: RowLayout {
                                spacing: 8
                                IconImage {
                                    source: "qrc:/lootingglass/icons/" + navItem.modelData.icon + ".svg"
                                    color: win.palette.windowText
                                    sourceSize: Qt.size(16, 16)
                                    Layout.preferredWidth: 16
                                    Layout.preferredHeight: 16
                                }
                                Label { text: navItem.text; Layout.fillWidth: true }
                            }
                            // Same selected look as checked filter rows in the overlay.
                            leftInset: -8
                            background: AccentTint {
                                accent: win.palette.highlight
                                bar: theme.selectionBar
                                active: navItem.ListView.isCurrentItem
                                hovered: navItem.hovered
                            }
                            onClicked: nav.currentIndex = index
                        }
                    }
                }
                Rectangle { Layout.fillHeight: true; implicitWidth: 1; color: win.palette.mid; opacity: 0.5 }

                // ── Pages ────────────────────────────────────────────
                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 8
                    currentIndex: win.categories.findIndex(c => c.name === (win.visibleCategories[nav.currentIndex] || {}).name)

                    FormPage {
                        FormLabel { text: "Price check against:" }
                        ComboBox {
                            id: game
                            model: ["Path of Exile", "Path of Exile 2"]
                        }
                        FormLabel { text: "PoE 1 league:" }
                        ComboBox {
                            id: league1
                            editable: true
                            model: win.leaguesFor("poe1")
                            Layout.preferredWidth: 260
                        }
                        FormLabel { text: "PoE 2 league:" }
                        ComboBox {
                            id: league2
                            editable: true
                            model: win.leaguesFor("poe2")
                            Layout.preferredWidth: 260
                        }
                        FormLabel { text: "Item info opens:" }
                        ComboBox {
                            id: infoSite
                            // Per game: poedb.tw / poe2db.tw, poewiki.net / poe2wiki.net.
                            model: ["poedb", "Community wiki"]
                            Layout.preferredWidth: 260
                        }
                        FormLabel { text: "Map check:"; visible: game.currentIndex === 0 }
                        LgCheckBox {
                            id: mapCheckOn
                            visible: game.currentIndex === 0
                            text: "Highlight deadly and annoying map mods"
                        }
                    }

                    FormPage {
                        FormLabel { text: "Listing status:" }
                        ComboBox {
                            id: status
                            model: ["Online", "Any", "Instant buyout (PoE 2)"]
                        }
                        FormLabel { text: "Stat tolerance (%):" }
                        SpinBox {
                            id: tolerance
                            from: 0; to: 50
                        }
                        Item { implicitWidth: 1 }
                        LgCheckBox {
                            id: minFromTier
                            text: "Use the bottom of the mod's tier instead (Ctrl+Alt+C)"
                        }
                        FormLabel { text: "Listings shown:" }
                        SpinBox {
                            id: results
                            from: 1; to: 20
                        }
                        FormLabel { text: "Contact (User-Agent):" }
                        TextField {
                            id: contact
                            placeholderText: "Optional, e.g. forum name"
                            Layout.preferredWidth: 260
                        }
                    }

                    FormPage {
                        FormLabel { text: "Screen side:" }
                        ComboBox {
                            id: side
                            model: ["Left", "Right"]
                        }
                        FormLabel { text: "Width:" }
                        SpinBox { id: width_; from: 250; to: 1500; stepSize: 10 }
                        FormLabel { text: "Height (0 = fit content):" }
                        SpinBox { id: maxHeight; from: 0; to: 3000; stepSize: 10 }
                        FormLabel { text: "Top margin:" }
                        SpinBox { id: top_; from: 0; to: 3000; stepSize: 10 }
                        FormLabel { text: "Side margin:" }
                        SpinBox { id: sideMargin; from: 0; to: 3000; stepSize: 10 }
                        Item {}
                        Label {
                            text: "Tip: drag the ⠿ handle to move the overlay and its free edges to resize it."
                            color: theme.secondaryText
                            wrapMode: Text.Wrap
                            Layout.preferredWidth: 300
                        }
                    }

                    AppearancePage {
                        id: appearancePage
                        appTheme: theme
                        onSaveAndRestart: if (win.save()) restartError = Backend.relaunch()
                    }

                    // How Looting Glass works with this desktop, then its shortcuts.
                    ColumnLayout {
                        id: scPage
                        spacing: 4
                        readonly property var desktop: {
                            try { return Backend.desktopInfo ? JSON.parse(Backend.desktopInfo) : null } catch (e) { return null }
                        }
                        readonly property var sc: {
                            try { return Backend.shortcuts ? JSON.parse(Backend.shortcuts) : null } catch (e) { return null }
                        }
                        readonly property var rows: sc ? sc.rows : []
                        readonly property bool editableHere: sc ? sc.editable_here : false
                        readonly property var bindHelp: sc ? sc.bind_help : ({ text: "", snippet: "", desktop_editor: false })

                        Repeater {
                            model: scPage.desktop ? scPage.desktop.missing : []
                            Label {
                                required property string modelData
                                text: modelData
                                color: theme.caution
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                                Layout.bottomMargin: 6
                            }
                        }

                        // Windows: Looting Glass registers the hotkeys itself, so they're typed here
                        // and saved with the other settings.
                        FormPage {
                            visible: win.ownShortcuts
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Repeater {
                                id: shortcutRows
                                model: win.ownShortcuts ? win.shortcutFields : []
                                delegate: RowLayout {
                                    required property var modelData
                                    property alias value: field.text
                                    Layout.columnSpan: 2
                                    Layout.alignment: Qt.AlignRight
                                    spacing: 12
                                    Label { text: modelData[1] }
                                    TextField {
                                        id: field
                                        placeholderText: "unassigned"
                                        Layout.preferredWidth: 200
                                    }
                                }
                            }
                            Item {}
                            Label {
                                text: scPage.bindHelp.text
                                color: theme.secondaryText
                                wrapMode: Text.Wrap
                                Layout.preferredWidth: 200
                            }
                        }

                        // Linux: the desktop owns global shortcuts. The shortcuts provider says how
                        // they're bound: edited right here (KDE), in the desktop's own editor, or as
                        // lines in the window manager's config.
                        ColumnLayout {
                            visible: !win.ownShortcuts
                            spacing: 4
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            Label {
                                text: scPage.bindHelp.text
                                wrapMode: Text.Wrap
                                color: theme.secondaryText
                                Layout.fillWidth: true
                                Layout.bottomMargin: 6
                            }
                            Repeater {
                                model: scPage.bindHelp.snippet === "" ? scPage.rows : []
                                ColumnLayout {
                                    id: scRow
                                    required property var modelData
                                    readonly property var pending: win.pendingShortcuts[modelData.id]
                                    readonly property string keys: pending ? pending.keys : modelData.keys
                                    Layout.fillWidth: true
                                    spacing: 0
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8
                                        Label {
                                            text: scRow.modelData.description
                                            elide: Text.ElideRight
                                            Layout.fillWidth: true
                                        }
                                        KeyButton {
                                            visible: scPage.editableHere
                                            shortcutId: scRow.modelData.id
                                            keys: scRow.keys
                                            changed: scRow.pending !== undefined && scRow.pending.keys !== scRow.modelData.keys
                                        }
                                        ToolButton {
                                            visible: scPage.editableHere
                                            enabled: scRow.keys !== ""
                                            opacity: enabled ? 1 : 0
                                            icon.name: "edit-clear"
                                            icon.source: "qrc:/lootingglass/icons/close.svg"
                                            ToolTip.visible: hovered
                                            ToolTip.text: "Remove this shortcut"
                                            onClicked: win.stageShortcut(scRow.modelData.id, 0, "", "")
                                        }
                                        Label {
                                            visible: !scPage.editableHere
                                            text: scRow.modelData.keys || "unassigned"
                                            color: scRow.modelData.keys ? win.palette.windowText : theme.faintText
                                        }
                                    }
                                    Label {
                                        visible: scRow.pending !== undefined && scRow.pending.conflict !== ""
                                        text: scRow.pending ? scRow.pending.keys + " is used by " + scRow.pending.conflict + ". Save moves it here." : ""
                                        color: theme.caution
                                        font.pointSize: 8
                                        wrapMode: Text.Wrap
                                        Layout.fillWidth: true
                                    }
                                }
                            }
                            TextArea {
                                visible: text !== ""
                                readOnly: true
                                selectByMouse: true
                                font.family: "monospace"
                                wrapMode: TextEdit.NoWrap
                                Layout.fillWidth: true
                                text: scPage.bindHelp.snippet
                            }
                            Label {
                                visible: text !== ""
                                text: Backend.shortcutError || (scPage.sc ? scPage.sc.error : "")
                                color: theme.danger
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                                Layout.topMargin: 6
                            }
                            Button {
                                visible: scPage.bindHelp.desktop_editor
                                text: "Change shortcuts…"
                                icon.name: "input-keyboard"
                                onClicked: Backend.configureShortcuts()
                                Layout.topMargin: 6
                            }
                            Item { Layout.fillHeight: true }
                        }
                        Label {
                            visible: text !== ""
                            text: scPage.desktop ? scPage.desktop.summary : ""
                            color: theme.secondaryText
                            font.pointSize: 8
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                    }

                    ColumnLayout {
                        spacing: 8
                        Label { text: "Marked map mods"; font.bold: true }
                        Label {
                            text: "The map check hotkey colours these mods on Path of Exile maps. Mark them from the overlay by clicking a mod, or paste a mod line here."
                            wrapMode: Text.Wrap
                            color: theme.secondaryText
                            Layout.fillWidth: true
                        }
                        RowLayout {
                            spacing: 8
                            TextField {
                                id: newMapMod
                                placeholderText: "e.g. Monsters reflect 18% of Elemental Damage"
                                Layout.fillWidth: true
                            }
                            // Numbers become `#`, so the mark covers every roll of the mod.
                            Button {
                                text: "Deadly"
                                enabled: newMapMod.text.trim() !== ""
                                onClicked: { win.setMapMark(newMapMod.text.trim().replace(/\d+(\.\d+)?/g, "#"), "deadly"); newMapMod.clear() }
                            }
                            Button {
                                text: "Annoying"
                                enabled: newMapMod.text.trim() !== ""
                                onClicked: { win.setMapMark(newMapMod.text.trim().replace(/\d+(\.\d+)?/g, "#"), "annoying"); newMapMod.clear() }
                            }
                        }
                        ScrollView {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            contentWidth: availableWidth
                            ColumnLayout {
                                width: parent.width
                                spacing: 2
                                Repeater {
                                    model: {
                                        const m = win.mapMods()
                                        return m ? m.deadly.map(t => [t, "deadly"]).concat(m.annoying.map(t => [t, "annoying"])) : []
                                    }
                                    RowLayout {
                                        required property var modelData
                                        Layout.fillWidth: true
                                        spacing: 8
                                        Label {
                                            text: modelData[1] === "deadly" ? "DEADLY" : "ANNOYING"
                                            font.pointSize: 8
                                            font.bold: true
                                            color: modelData[1] === "deadly" ? theme.danger : theme.caution
                                            Layout.preferredWidth: 70
                                        }
                                        Label { text: modelData[0]; elide: Text.ElideRight; Layout.fillWidth: true }
                                        ToolButton {
                                            text: modelData[1] === "deadly" ? "Annoying" : "Deadly"
                                            onClicked: win.setMapMark(modelData[0], modelData[1] === "deadly" ? "annoying" : "deadly")
                                        }
                                        ToolButton {
                                            text: "Remove"
                                            onClicked: win.setMapMark(modelData[0], "")
                                        }
                                    }
                                }
                                Label {
                                    visible: { const m = win.mapMods(); return !m || m.deadly.length + m.annoying.length === 0 }
                                    text: "No marked mods yet."
                                    color: theme.faintText
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        spacing: 8
                        Label { text: "Looting Glass"; font.pointSize: Qt.application.font.pointSize * 1.6 }
                        Label { text: "Version " + Backend.version; color: theme.secondaryText }

                        // Update notice: release notes and, where this copy can update itself, Install.
                        Frame {
                            visible: Backend.updateVersion !== ""
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            ColumnLayout {
                                anchors.fill: parent
                                spacing: 6
                                Label {
                                    text: "Looting Glass " + Backend.updateVersion + " is available"
                                    font.bold: true
                                }
                                ScrollView {
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    Layout.minimumHeight: 40
                                    clip: true
                                    Label {
                                        width: parent.width
                                        text: Backend.updateNotes
                                        textFormat: Text.MarkdownText
                                        wrapMode: Text.Wrap
                                        onLinkActivated: link => Qt.openUrlExternally(link)
                                    }
                                }
                                Label {
                                    visible: Backend.updateMode !== "install"
                                    text: Backend.updateMode === "package"
                                        ? "Update it with your package manager."
                                        : "Download it from the release page."
                                    color: theme.secondaryText
                                }
                                RowLayout {
                                    Button {
                                        visible: Backend.updateMode === "install" && !Backend.updateReady
                                        enabled: !Backend.updateBusy
                                        text: "Install update"
                                        icon.name: theme.desktopIcons ? "system-software-update" : ""
                                        highlighted: true
                                        onClicked: Backend.installUpdate()
                                    }
                                    Button {
                                        visible: Backend.updateReady
                                        text: "Restart now"
                                        icon.name: theme.desktopIcons ? "view-refresh" : ""
                                        highlighted: true
                                        onClicked: Backend.restartApp()
                                    }
                                    Button {
                                        text: "Release page"
                                        icon.name: theme.desktopIcons ? "internet-services" : ""
                                        onClicked: Qt.openUrlExternally(Backend.updateUrl)
                                    }
                                }
                            }
                        }

                        LgCheckBox {
                            id: checkUpdates
                            text: "Check for updates automatically"
                        }
                        RowLayout {
                            spacing: 8
                            Button {
                                text: "Check for updates"
                                enabled: !Backend.updateBusy
                                onClicked: Backend.checkUpdates()
                            }
                            Label {
                                text: Backend.updateStatus !== "" ? Backend.updateStatus
                                    : Backend.lastChecked !== "" ? "Last checked " + Backend.lastChecked : ""
                                color: theme.secondaryText
                                wrapMode: Text.Wrap
                                Layout.fillWidth: true
                            }
                        }
                        Label {
                            text: "Wayland-native price checker for Path of Exile 1 and 2."
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        Label {
                            text: "Looting Glass is a fan-made tool, not affiliated with or endorsed by Grinding Gear Games. Licensed under GPL-3.0-or-later."
                            color: theme.secondaryText
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        Label {
                            visible: Backend.snapshotDir === ""
                            text: "Config file: " + Backend.configPath()
                            color: theme.secondaryText
                            wrapMode: Text.WrapAnywhere
                            Layout.fillWidth: true
                        }
                        Item { Layout.fillHeight: true }
                    }
                }
            }

            // ── Footer ───────────────────────────────────────────────
            Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: win.palette.mid; opacity: 0.5 }
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 8
                Label { id: saveStatus; color: theme.secondaryText; Layout.fillWidth: true }
                Button { text: "Save"; icon.name: theme.desktopIcons ? "document-save" : ""; highlighted: true; onClicked: win.save() }
                Button { text: "Close"; icon.name: theme.desktopIcons ? "dialog-close" : ""; onClicked: win.close() }
            }
        }
    }
}
