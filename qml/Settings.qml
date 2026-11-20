import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.lootlens

ApplicationWindow {
    id: win
    title: "Loot Lens Settings"
    width: 560
    height: 680
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

    ScrollView {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        contentWidth: availableWidth

        Kirigami.FormLayout {
            width: parent.width

            Kirigami.Separator { Kirigami.FormData.label: "Game"; Kirigami.FormData.isSection: true }
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
                Layout.fillWidth: true
            }
            ComboBox {
                id: league2
                Kirigami.FormData.label: "PoE 2 league:"
                editable: true
                model: win.leaguesFor("poe2")
                Layout.fillWidth: true
            }

            Kirigami.Separator { Kirigami.FormData.label: "Trade"; Kirigami.FormData.isSection: true }
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
                Layout.fillWidth: true
            }

            Kirigami.Separator { Kirigami.FormData.label: "Overlay"; Kirigami.FormData.isSection: true }
            ComboBox {
                id: side
                Kirigami.FormData.label: "Screen side:"
                model: ["Left", "Right"]
            }
            SpinBox { id: width_; Kirigami.FormData.label: "Width:"; from: 250; to: 900; stepSize: 10 }
            SpinBox { id: top_; Kirigami.FormData.label: "Top margin:"; from: 0; to: 1500; stepSize: 10 }
            SpinBox { id: sideMargin; Kirigami.FormData.label: "Side margin:"; from: 0; to: 1500; stepSize: 10 }

            Kirigami.Separator { Kirigami.FormData.label: "Shortcuts"; Kirigami.FormData.isSection: true }
            Label {
                Kirigami.FormData.label: "Current:"
                text: Backend.shortcutsText || "…"
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Button {
                text: "Change shortcuts…"
                icon.name: "input-keyboard"
                onClicked: Backend.configureShortcuts()
            }

            Kirigami.Separator { Kirigami.FormData.isSection: true }
            RowLayout {
                Button { text: "Save"; icon.name: "document-save"; highlighted: true; onClicked: win.save() }
                Button { text: "Close"; icon.name: "dialog-close"; onClicked: win.close() }
                Label { id: saveStatus; opacity: 0.8 }
            }
            Label {
                text: "Loot Lens is a fan-made tool, not affiliated with or endorsed by Grinding Gear Games."
                opacity: 0.6
                font.pointSize: 8
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Label {
                text: "Config file: " + Backend.configPath()
                opacity: 0.6
                font.pointSize: 8
                wrapMode: Text.WrapAnywhere
                Layout.fillWidth: true
            }
        }
    }
}
