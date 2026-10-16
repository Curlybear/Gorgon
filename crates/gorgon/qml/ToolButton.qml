import QtQuick

// A flat toolbar button; no QtQuick.Controls so nothing extra needs installing.
Rectangle {
    id: button

    property string label
    property string hint
    property bool checked: false
    property bool enabled: true
    signal clicked()

    implicitWidth: Math.max(32, text.implicitWidth + 16)
    implicitHeight: 32
    radius: 6
    color: checked ? "#3d6fd9" : (mouse.containsMouse && enabled ? "#3a3d44" : "transparent")
    opacity: enabled ? 1 : 0.4

    Text {
        id: text
        anchors.centerIn: parent
        text: button.label
        color: "#f0f0f0"
        font.pixelSize: 14
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: if (button.enabled) button.clicked()
    }

    Rectangle {
        visible: mouse.containsMouse && button.hint !== ""
        z: 10
        anchors.top: parent.bottom
        anchors.topMargin: 6
        anchors.horizontalCenter: parent.horizontalCenter
        width: hintText.implicitWidth + 12
        height: hintText.implicitHeight + 8
        radius: 4
        color: "#000000"

        Text {
            id: hintText
            anchors.centerIn: parent
            text: button.hint
            color: "#f0f0f0"
            font.pixelSize: 12
        }
    }
}
