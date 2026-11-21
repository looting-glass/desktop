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
    KdeLook { target: win.palette }
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
    readonly property var infoSites: ["wiki", "poedb"]

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
        syncLeague(league1, cfg.poe1.league)
        syncLeague(league2, cfg.poe2.league)
        infoSite.currentIndex = Math.max(0, infoSites.indexOf(cfg.links.site))
        status.currentIndex = Math.max(0, statuses.indexOf(cfg.trade.status))
        tolerance.value = cfg.trade.stat_tolerance
        results.value = cfg.trade.max_results
        side.currentIndex = cfg.overlay.side === "left" ? 0 : 1
        width_.value = cfg.overlay.width
        top_.value = cfg.overlay.margin_top
        sideMargin.value = cfg.overlay.margin_side
        maxHeight.value = cfg.overlay.max_height
        contact.text = cfg.contact
        checkUpdates.checked = cfg.updates.check
        if (ownShortcuts)
            for (let i = 0; i < shortcutFields.length; i++)
                shortcutRows.itemAt(i).value = cfg.shortcuts[shortcutFields[i][0]]
        saveStatus.text = ""
        Backend.refreshLeagues()
        Backend.refreshShortcuts()
    }
    function save() {
        const c = cfg
        c.game = game.currentIndex === 1 ? "poe2" : "poe1"
        c.poe1.league = league1.editText.trim() || "Standard"
        c.poe2.league = league2.editText.trim() || "Standard"
        c.links.site = infoSites[infoSite.currentIndex]
        c.trade.status = statuses[status.currentIndex]
        c.trade.stat_tolerance = tolerance.value
        c.trade.max_results = results.value
        c.overlay.side = side.currentIndex === 0 ? "left" : "right"
        c.overlay.width = width_.value
        c.overlay.margin_top = top_.value
        c.overlay.margin_side = sideMargin.value
        c.overlay.max_height = maxHeight.value
        c.contact = contact.text
        c.updates.check = checkUpdates.checked
        if (ownShortcuts)
            for (let i = 0; i < shortcutFields.length; i++)
                c.shortcuts[shortcutFields[i][0]] = shortcutRows.itemAt(i).value.trim()
        const err = Backend.saveConfig(JSON.stringify(c))
        saveStatus.text = err === "" ? "Saved." : "Error: " + err
    }

    onVisibleChanged: if (visible) load()

    function showAbout() {
        nav.currentIndex = categories.length - 1
        show()
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
    // saves the Game and Trade tabs as settings-game.png / settings-trade.png in `dir`.
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
        readonly property var shots: [["settings-game.png", 0], ["settings-trade.png", 1], ["settings-about.png", 4]]
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
        { name: "Shortcuts", icon: "nav-shortcuts" },
        { name: "About", icon: "nav-about" },
    ]

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
                        model: win.categories
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
                    currentIndex: nav.currentIndex

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
                            // Per game: poewiki.net / poe2wiki.net, poedb.tw / poe2db.tw.
                            model: ["Community wiki", "poedb"]
                            Layout.preferredWidth: 260
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
                            opacity: 0.7
                            wrapMode: Text.Wrap
                            Layout.preferredWidth: 300
                        }
                    }

                    // Linux: shortcuts live in the desktop (portal); show them with a button to edit.
                    // Windows: Looting Glass registers them itself, so edit them right here.
                    FormPage {
                        FormLabel { text: "Current:"; visible: !win.ownShortcuts }
                        Label {
                            visible: !win.ownShortcuts
                            text: Backend.shortcutsText || "…"
                            wrapMode: Text.Wrap
                            Layout.preferredWidth: 300
                        }
                        Item { visible: !win.ownShortcuts }
                        Button {
                            visible: !win.ownShortcuts
                            text: "Change shortcuts…"
                            icon.name: Qt.platform.os === "linux" ? "input-keyboard" : ""
                            onClicked: Backend.configureShortcuts()
                        }

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
                        Item { visible: win.ownShortcuts }
                        Label {
                            visible: win.ownShortcuts
                            text: "Examples: Ctrl+D, Ctrl+Shift+F7, Alt+W. Leave empty to unassign."
                            opacity: 0.7
                            wrapMode: Text.Wrap
                            Layout.preferredWidth: 200
                        }
                    }

                    ColumnLayout {
                        spacing: 8
                        Label { text: "Looting Glass"; font.pointSize: Qt.application.font.pointSize * 1.6 }
                        Label { text: "Version " + Backend.version; opacity: 0.7 }

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
                                    opacity: 0.7
                                }
                                RowLayout {
                                    Button {
                                        visible: Backend.updateMode === "install" && !Backend.updateReady
                                        enabled: !Backend.updateBusy
                                        text: "Install update"
                                        icon.name: Qt.platform.os === "linux" ? "system-software-update" : ""
                                        highlighted: true
                                        onClicked: Backend.installUpdate()
                                    }
                                    Button {
                                        visible: Backend.updateReady
                                        text: "Restart now"
                                        icon.name: Qt.platform.os === "linux" ? "view-refresh" : ""
                                        highlighted: true
                                        onClicked: Backend.restartApp()
                                    }
                                    Button {
                                        text: "Release page"
                                        icon.name: Qt.platform.os === "linux" ? "internet-services" : ""
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
                                opacity: 0.7
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
                            opacity: 0.7
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        Label {
                            visible: Backend.snapshotDir === ""
                            text: "Config file: " + Backend.configPath()
                            opacity: 0.7
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
                Label { id: saveStatus; opacity: 0.8; Layout.fillWidth: true }
                Button { text: "Save"; icon.name: Qt.platform.os === "linux" ? "document-save" : ""; highlighted: true; onClicked: win.save() }
                Button { text: "Close"; icon.name: Qt.platform.os === "linux" ? "dialog-close" : ""; onClicked: win.close() }
            }
        }
    }
}
