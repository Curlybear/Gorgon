import QtQuick
import dev.gorgon.spike

// No window of its own: overlays are created per screen while selecting.
QtObject {
    id: root

    property bool selecting: false
    property bool inTray: false

    // Logical bounds of the whole desktop, to map screens onto the screenshot.
    readonly property var screens: Array.from(Qt.application.screens)
    readonly property real minX: Math.min(...screens.map(s => s.virtualX))
    readonly property real minY: Math.min(...screens.map(s => s.virtualY))
    readonly property real desktopWidth:
        Math.max(...screens.map(s => s.virtualX + s.width)) - minX

    property Capture capture: Capture {
        onCaptured: {
            console.log("image is", imageWidth, "px wide, desktop is", root.desktopWidth,
                        "logical px wide, scale", imageWidth / root.desktopWidth)
            root.selecting = true
        }
    }

    property Instantiator overlays: Instantiator {
        model: root.selecting ? Qt.application.screens : []
        delegate: Overlay {
            required property var modelData
            targetScreen: modelData
            capture: root.capture
            originX: root.minX
            originY: root.minY
            imageScale: root.capture.imageWidth / root.desktopWidth
            onFinished: {
                root.selecting = false
                // Without the tray there is nothing left to do.
                if (!root.inTray)
                    Qt.quit()
            }
        }
    }

    function startCapture() {
        if (!selecting)
            capture.capture()
    }

    Component.onCompleted: {
        if (Qt.application.arguments.indexOf("--tray") >= 0) {
            inTray = capture.startTray()
            if (!inTray)
                Qt.quit()
        } else {
            startCapture()
        }
    }
}
