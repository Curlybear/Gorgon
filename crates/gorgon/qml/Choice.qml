import QtQuick

// One option of a group: a round marker and a label.
Item {
    id: root

    property string label
    property bool selected: false
    property bool enabled: true
    signal picked()

    implicitWidth: row.implicitWidth
    implicitHeight: 28
    opacity: enabled ? 1 : 0.45

    Row {
        id: row
        spacing: 10
        anchors.verticalCenter: parent.verticalCenter

        Rectangle {
            width: 18
            height: 18
            radius: 9
            anchors.verticalCenter: parent.verticalCenter
            color: "transparent"
            border.color: root.selected ? "#3d6fd9" : "#6b6f77"
            border.width: 1.5

            Rectangle {
                anchors.centerIn: parent
                width: 10
                height: 10
                radius: 5
                color: "#3d6fd9"
                visible: root.selected
            }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: "#f0f0f0"
            font.pixelSize: 14
        }
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: if (root.enabled) root.picked()
    }
}
