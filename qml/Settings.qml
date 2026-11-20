import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.lootlens

// Settings window: category sidebar (like KDE System Settings) + one short form per page.
ApplicationWindow {
    id: win
    title: "Loot Lens Settings"
    width: 760
    height: 500
    minimumWidth: 620
    minimumHeight: 420
    visible: false

    property var cfg: ({})
    readonly property var statuses: ["online", "any", "securable"]

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
        status.currentIndex = Math.max(0, statuses.indexOf(cfg.trade.status))
        tolerance.value = cfg.trade.stat_tolerance
        results.value = cfg.trade.max_results
        side.currentIndex = cfg.overlay.side === "left" ? 0 : 1
        width_.value = cfg.overlay.width
        top_.value = cfg.overlay.margin_top
        sideMargin.value = cfg.overlay.margin_side
        maxHeight.value = cfg.overlay.max_height
        contact.text = cfg.contact
        saveStatus.text = ""
        Backend.refreshLeagues()
        Backend.refreshShortcuts()
    }
    function save() {
        const c = cfg
        c.game = game.currentIndex === 1 ? "poe2" : "poe1"
        c.poe1.league = league1.editText.trim() || "Standard"
        c.poe2.league = league2.editText.trim() || "Standard"
        c.trade.status = statuses[status.currentIndex]
        c.trade.stat_tolerance = tolerance.value
        c.trade.max_results = results.value
        c.overlay.side = side.currentIndex === 0 ? "left" : "right"
        c.overlay.width = width_.value
        c.overlay.margin_top = top_.value
        c.overlay.margin_side = sideMargin.value
        c.overlay.max_height = maxHeight.value
        c.contact = contact.text
        const err = Backend.saveConfig(JSON.stringify(c))
        saveStatus.text = err === "" ? "Saved." : "Error: " + err
    }

    onVisibleChanged: if (visible) load()

    Connections {
        target: Backend
        function onLeaguesChanged() {
            win.syncLeague(league1, win.cfg.poe1.league)
            win.syncLeague(league2, win.cfg.poe2.league)
        }
    }

    // Page background as a real item so it can be captured (LOOTLENS_SNAPSHOT).
    function snapshot(path) {
        win.width = 760
        win.height = 440
        nav.currentIndex = 1
        snapTimer.path = path
        snapTimer.start()
    }
    Timer {
        id: snapTimer
        property string path
        interval: 800
        onTriggered: page.grabToImage(r => {
            r.saveToFile(path)
            console.warn("snapshot: settings saved")
        })
    }

    readonly property var categories: [
        { name: "Game", icon: "applications-games" },
        { name: "Trade", icon: "view-financial-list" },
        { name: "Overlay", icon: "window" },
        { name: "Shortcuts", icon: "input-keyboard" },
        { name: "About", icon: "help-about" },
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
                        anchors.margins: Kirigami.Units.smallSpacing
                        model: win.categories
                        currentIndex: 0
                        spacing: 2
                        delegate: ItemDelegate {
                            required property var modelData
                            required property int index
                            width: ListView.view.width
                            text: modelData.name
                            icon.name: modelData.icon
                            highlighted: ListView.isCurrentItem
                            onClicked: nav.currentIndex = index
                        }
                    }
                }
                Kirigami.Separator { Layout.fillHeight: true }

                // ── Pages ────────────────────────────────────────────
                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    currentIndex: nav.currentIndex

                    Kirigami.FormLayout {
                        ComboBox {
                            id: game
                            Kirigami.FormData.label: "Price check against:"
                            model: ["Path of Exile", "Path of Exile 2"]
                        }
                        ComboBox {
                            id: league1
                            Kirigami.FormData.label: "PoE 1 league:"
                            editable: true
                            model: win.leaguesFor("poe1")
                            Layout.preferredWidth: 260
                        }
                        ComboBox {
                            id: league2
                            Kirigami.FormData.label: "PoE 2 league:"
                            editable: true
                            model: win.leaguesFor("poe2")
                            Layout.preferredWidth: 260
                        }
                    }

                    Kirigami.FormLayout {
                        ComboBox {
                            id: status
                            Kirigami.FormData.label: "Listing status:"
                            model: ["Online", "Any", "Instant buyout (PoE 2)"]
                        }
                        SpinBox {
                            id: tolerance
                            Kirigami.FormData.label: "Stat tolerance (%):"
                            from: 0; to: 50
                        }
                        SpinBox {
                            id: results
                            Kirigami.FormData.label: "Listings shown:"
                            from: 1; to: 20
                        }
                        TextField {
                            id: contact
                            Kirigami.FormData.label: "Contact (User-Agent):"
                            placeholderText: "Optional, e.g. forum name"
                            Layout.preferredWidth: 260
                        }
                    }

                    Kirigami.FormLayout {
                        ComboBox {
                            id: side
                            Kirigami.FormData.label: "Screen side:"
                            model: ["Left", "Right"]
                        }
                        SpinBox { id: width_; Kirigami.FormData.label: "Width:"; from: 250; to: 1500; stepSize: 10 }
                        SpinBox { id: maxHeight; Kirigami.FormData.label: "Height (0 = fit content):"; from: 0; to: 3000; stepSize: 10 }
                        SpinBox { id: top_; Kirigami.FormData.label: "Top margin:"; from: 0; to: 3000; stepSize: 10 }
                        SpinBox { id: sideMargin; Kirigami.FormData.label: "Side margin:"; from: 0; to: 3000; stepSize: 10 }
                        Label {
                            text: "Tip: drag the ⠿ handle to move the overlay and its free edges to resize it."
                            opacity: 0.7
                            wrapMode: Text.Wrap
                            Layout.preferredWidth: 300
                        }
                    }

                    Kirigami.FormLayout {
                        Label {
                            Kirigami.FormData.label: "Current:"
                            text: Backend.shortcutsText || "…"
                            wrapMode: Text.Wrap
                            Layout.preferredWidth: 300
                        }
                        Button {
                            text: "Change shortcuts…"
                            icon.name: "input-keyboard"
                            onClicked: Backend.configureShortcuts()
                        }
                    }

                    ColumnLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Kirigami.Heading { text: "Loot Lens"; level: 1 }
                        Label {
                            text: "Wayland-native price checker for Path of Exile 1 and 2."
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        Label {
                            text: "Loot Lens is a fan-made tool, not affiliated with or endorsed by Grinding Gear Games. Licensed under GPL-3.0-or-later."
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
            Kirigami.Separator { Layout.fillWidth: true }
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.largeSpacing
                Label { id: saveStatus; opacity: 0.8; Layout.fillWidth: true }
                Button { text: "Save"; icon.name: "document-save"; highlighted: true; onClicked: win.save() }
                Button { text: "Close"; icon.name: "dialog-close"; onClicked: win.close() }
            }
        }
    }
}
