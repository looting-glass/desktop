import QtQuick
import QtQuick.Layouts

// Two-column settings form (right-aligned labels, controls on the right), centred at the top
// of its page. Put a FormLabel before each control; full-width rows use Layout.columnSpan: 2.
Item {
    default property alias content: grid.data
    GridLayout {
        id: grid
        anchors.top: parent.top
        anchors.topMargin: 8
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(implicitWidth, parent.width)
        columns: 2
        columnSpacing: 12
        rowSpacing: 8
    }
}
