import QtQuick

// A single-line text field.
Rectangle {
    id: root

    property alias text: input.text
    property string placeholder
    signal accepted()

    function edit() { input.forceActiveFocus() }

    implicitWidth: 320
    implicitHeight: 32
    radius: 6
    color: "#1e1f22"
    border.color: input.activeFocus ? "#3d6fd9" : "#3a3d44"

    TextInput {
        id: input
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        verticalAlignment: TextInput.AlignVCenter
        color: "#f0f0f0"
        font.pixelSize: 14
        selectByMouse: true
        clip: true
        onAccepted: root.accepted()

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: input.text === "" && !input.activeFocus
            text: root.placeholder
            color: "#6b6f77"
            font.pixelSize: 14
        }
    }
}
