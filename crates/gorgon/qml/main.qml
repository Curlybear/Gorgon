import QtQuick
import dev.gorgon

// The daemon has no window of its own: overlays appear per screen during a capture.
QtObject {
    id: root

    property bool selecting: false
    property bool flashing: false
    property bool flashHandled: false
    property bool grabbing: false

    // Logical bounds of the whole desktop, to map screens onto the screenshot.
    readonly property var screens: Array.from(Qt.application.screens)
    readonly property real minX: Math.min(...screens.map(s => s.virtualX))
    readonly property real minY: Math.min(...screens.map(s => s.virtualY))
    readonly property real desktopWidth:
        Math.max(...screens.map(s => s.virtualX + s.width)) - minX

    property App app: App {
        onSelectRegion: root.selecting = true
        onOpenEditor: editorList.append({})
        onCopyRequested: root.grabbing = true
        onShowWindow: name => root.showWindow(name)
        onFlash: {
            root.flashHandled = false
            root.flashing = true
        }
    }

    property Instantiator regionOverlays: Instantiator {
        model: root.selecting ? root.screens : []
        delegate: RegionOverlay {
            required property var modelData
            targetScreen: modelData
            app: root.app
            originX: root.minX
            originY: root.minY
            imageScale: root.app.imageWidth / root.desktopWidth
            onDone: root.selecting = false
        }
    }

    property ListModel editorList: ListModel {}

    property Instantiator editors: Instantiator {
        model: root.editorList
        delegate: EditorWindow {
            required property int index
            onClosed: Qt.callLater(() => root.editorList.remove(index))
        }
    }

    // One History and one Settings window at most; asking again raises it.
    property var openWindows: ({})
    property Component historyWindow: Component { HistoryWindow {} }
    property Component settingsWindow: Component { SettingsWindow {} }

    function showWindow(name) {
        let w = openWindows[name]
        if (!w) {
            const component = name === "history" ? historyWindow : settingsWindow
            w = name === "history" ? component.createObject(null, { app: root.app })
                                   : component.createObject(null)
            if (!w)
                return
            openWindows[name] = w
            w.closed.connect(() => {
                delete openWindows[name]
                Qt.callLater(() => w.destroy())
            })
        }
        w.show()
        w.raise()
        w.requestActivate()
    }

    property Instantiator clipboardGrab: Instantiator {
        model: root.grabbing ? 1 : 0
        delegate: ClipboardGrab {
            app: root.app
            onDone: root.grabbing = false
        }
    }

    property Instantiator flashes: Instantiator {
        model: root.flashing ? root.screens : []
        delegate: Flash {
            required property var modelData
            targetScreen: modelData
            onFocused: root.finishFlash()
        }
    }

    // The clipboard needs a focused window; don't wait forever for one.
    property Timer flashFallback: Timer {
        interval: 600
        running: root.flashing && !root.flashHandled
        onTriggered: root.finishFlash()
    }

    property Timer flashEnd: Timer {
        interval: 250
        onTriggered: root.flashing = false
    }

    function finishFlash() {
        if (flashHandled)
            return
        flashHandled = true
        app.finishFull()
        flashEnd.restart()
    }
}
