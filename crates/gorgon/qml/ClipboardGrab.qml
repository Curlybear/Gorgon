import QtQuick
import QtQuick.Window

// A tiny, invisible window that exists only to get keyboard focus, which
// Wayland requires before an app may set the clipboard.
Window {
    id: win

    property var app
    signal done()
    property bool handled: false

    width: 1
    height: 1
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"
    title: "Gorgon"

    function finish(ok) {
        if (handled)
            return
        handled = true
        ok ? app.copyPending() : app.copyFailed()
        // Give the clipboard a moment to be claimed before the window goes.
        closeTimer.start()
    }

    Component.onCompleted: {
        show()
        requestActivate()
    }
    onActiveChanged: if (active) finish(true)

    Timer {
        interval: 800
        running: true
        onTriggered: win.finish(false)
    }
    Timer {
        id: closeTimer
        interval: 150
        onTriggered: win.done()
    }
}
