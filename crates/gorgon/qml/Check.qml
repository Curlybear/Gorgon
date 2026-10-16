import QtQuick

// A labelled checkbox; no QtQuick.Controls needed.
Item {
    id: root

    property string label
    property string detail
    property bool checked: false
    property bool enabled: true
    signal toggled(bool checked)

    implicitWidth: row.implicitWidth
    implicitHeight: Math.max(28, row.implicitHeight)
    opacity: enabled ? 1 : 0.45

    Row {
        id: row
        spacing: 10
        anchors.verticalCenter: parent.verticalCenter

        Rectangle {
            width: 18
            height: 18
            radius: 4
            anchors.verticalCenter: parent.verticalCenter
            color: root.checked ? "#3d6fd9" : "transparent"
            border.color: root.checked ? "#3d6fd9" : "#6b6f77"
            border.width: 1.5

            Text {
                anchors.centerIn: parent
                visible: root.checked
                text: "✓"
                color: "white"
                font.pixelSize: 13
            }
        }
        Column {
            anchors.verticalCenter: parent.verticalCenter
            Text {
                text: root.label
                color: "#f0f0f0"
                font.pixelSize: 14
            }
            Text {
                visible: root.detail !== ""
                text: root.detail
                color: "#a0a4ab"
                font.pixelSize: 12
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: if (root.enabled) {
            root.checked = !root.checked
            root.toggled(root.checked)
        }
    }
}
