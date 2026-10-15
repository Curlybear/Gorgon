import QtQuick
import Qt.labs.platform

SystemTrayIcon {
    signal captureRequested()

    visible: true
    icon.name: "camera-photo"
    tooltip: "Gorgon spike"
    Component.onCompleted: console.log("tray available:", available)
    onActivated: captureRequested()

    menu: Menu {
        MenuItem {
            text: "Capture region"
            onTriggered: captureRequested()
        }
        MenuItem {
            text: "Quit"
            onTriggered: Qt.quit()
        }
    }
}
