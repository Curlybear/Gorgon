import QtQuick
import QtQuick.Window

// A brief white flash over a screen, confirming a full capture.
Window {
    id: win

    property var targetScreen
    signal focused()

    screen: targetScreen
    x: targetScreen.virtualX
    y: targetScreen.virtualY
    width: targetScreen.width
    height: targetScreen.height
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"
    title: "Gorgon"

    Component.onCompleted: showFullScreen()
    onActiveChanged: if (active) focused()

    Rectangle {
        anchors.fill: parent
        color: "white"
        NumberAnimation on opacity {
            from: 0.5
            to: 0
            duration: 250
        }
    }
}
