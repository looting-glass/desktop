// A drag of the overlay's handle or one of its resize edges: the overlay moves or resizes live,
// so the grabbed spot stays under the cursor. Let go over another monitor and it moves there.
//
// Positions are worked out on the overlay's monitor, from where the drag started and how far the
// cursor has moved since. On Linux the window covers the monitor and never moves (cpp/main.cpp),
// so positions in it are already the monitor's; elsewhere the window moves with the overlay, so
// they come from the cursor's position on the desktop.
import QtQuick
import dev.lootingglass

QtObject {
    required property Window window
    // The drag handle: a drag of it can end on another monitor, a resize can't.
    required property Item handle

    // The handle or edge being dragged, or null. Its dragTo(dx, dy) moves or resizes the overlay
    // for the cursor moved that far since the press.
    property Item area: null
    property bool moving: false
    // The overlay's place and size when the drag started, and the press, on the monitor.
    property rect start
    property point press
    // The latest cursor position, which may be past the monitor's edges.
    property point cursor

    // The window's monitor, with its place among the others (window.screen has no virtualX).
    readonly property var monitor: Qt.application.screens.find(s => s.name === window.screen.name)

    function begin(dragged, mouse) {
        area = dragged
        moving = false
        start = Qt.rect(window.cardX, window.marginTop, window.overlayWidth, window.overlayHeight)
        press = cursor = onMonitor(dragged, mouse)
    }

    function follow(dragged, mouse) {
        if (area !== dragged) return
        cursor = onMonitor(dragged, mouse)
        const dx = cursor.x - press.x
        const dy = cursor.y - press.y
        // A click (to focus the handle, or a double-click on an edge) changes nothing.
        if (!moving && Math.hypot(dx, dy) < Qt.styleHints.startDragDistance) return
        moving = true
        area.dragTo(dx, dy)
    }

    // Released, or the pointer taken away mid-drag: the overlay stays where it was last shown,
    // or the handle's drag moves it to the monitor the cursor was let go over.
    function end(dragged) {
        if (area !== dragged) return
        area = null
        if (!moving) return
        const target = monitorUnder(cursor)
        if (dragged === handle && target.name !== monitor.name) {
            // Same spot under the cursor, now in the other monitor's coordinates.
            const x = monitor.virtualX + cursor.x - (press.x - start.x) - target.virtualX
            const y = monitor.virtualY + cursor.y - (press.y - start.y) - target.virtualY
            Backend.overlayScreen = target.name
            Backend.placeOverlay(Qt.point(x, y), Qt.size(target.width, target.height))
        }
        Backend.saveOverlay()
    }

    function onMonitor(item, mouse) {
        if (window.coversMonitor) return item.mapToItem(null, mouse.x, mouse.y)
        const p = item.mapToGlobal(mouse.x, mouse.y)
        return Qt.point(p.x - monitor.virtualX, p.y - monitor.virtualY)
    }

    // The monitor under a point on this one; this one when the point is between monitors.
    function monitorUnder(p) {
        const x = monitor.virtualX + p.x
        const y = monitor.virtualY + p.y
        return Qt.application.screens.find(s => x >= s.virtualX && x < s.virtualX + s.width
                                                && y >= s.virtualY && y < s.virtualY + s.height)
            ?? monitor
    }
}
