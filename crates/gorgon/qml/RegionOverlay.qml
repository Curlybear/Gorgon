import QtQuick
import QtQuick.Window
import dev.gorgon

// Covers one screen with the frozen screenshot and lets the user drag a region.
Window {
    id: win

    property var targetScreen
    property App app
    property real originX: 0
    property real originY: 0
    property real imageScale: 1
    signal done()

    // Where this screen sits inside the screenshot, in image pixels.
    readonly property real offsetX: (targetScreen.virtualX - originX) * imageScale
    readonly property real offsetY: (targetScreen.virtualY - originY) * imageScale

    screen: targetScreen
    x: targetScreen.virtualX
    y: targetScreen.virtualY
    width: targetScreen.width
    height: targetScreen.height
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "black"
    title: "Gorgon"

    Component.onCompleted: showFullScreen()

    function cancel() {
        app.cancel()
        done()
    }

    Image {
        anchors.fill: parent
        source: win.app.imageUrl
        sourceClipRect: Qt.rect(win.offsetX, win.offsetY,
                                win.width * win.imageScale, win.height * win.imageScale)
        cache: false
        smooth: false
    }

    SelectionPainter {
        id: painter
        anchors.fill: parent
    }

    // Crosshair, hidden while dragging.
    Rectangle {
        visible: mouse.containsMouse && !mouse.pressed
        x: 0
        y: Math.round(mouse.mouseY)
        width: parent.width
        height: 1
        color: "#8040a0ff"
    }
    Rectangle {
        visible: mouse.containsMouse && !mouse.pressed
        x: Math.round(mouse.mouseX)
        y: 0
        width: 1
        height: parent.height
        color: "#8040a0ff"
    }

    Text {
        visible: painter.selection.width > 0
        x: Math.min(painter.selection.x, win.width - width)
        y: painter.selection.y > height + 6 ? painter.selection.y - height - 4
                                             : painter.selection.y + painter.selection.height + 4
        color: "white"
        style: Text.Outline
        styleColor: "black"
        text: Math.round(painter.selection.width * win.imageScale) + " × "
              + Math.round(painter.selection.height * win.imageScale)
    }

    Text {
        visible: painter.selection.width === 0
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 48
        color: "white"
        style: Text.Outline
        styleColor: "black"
        font.pixelSize: 16
        text: (win.app.forRecording ? "Drag the area to record" : "Drag to capture") + " · Esc or right-click to cancel"
    }

    MouseArea {
        id: mouse

        property point start

        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.CrossCursor
        onPressed: event => {
            if (event.button === Qt.RightButton) {
                win.cancel()
                return
            }
            start = Qt.point(event.x, event.y)
            painter.selection = Qt.rect(event.x, event.y, 0, 0)
        }
        onPositionChanged: event => {
            if (!pressed)
                return
            painter.selection = Qt.rect(Math.min(start.x, event.x), Math.min(start.y, event.y),
                                        Math.abs(event.x - start.x), Math.abs(event.y - start.y))
        }
        onReleased: event => {
            if (event.button !== Qt.LeftButton)
                return
            const s = painter.selection
            if (s.width < 2 || s.height < 2) {
                painter.selection = Qt.rect(0, 0, 0, 0)
                return
            }
            win.app.finishRegion(win.offsetX + s.x * win.imageScale,
                                 win.offsetY + s.y * win.imageScale,
                                 s.width * win.imageScale,
                                 s.height * win.imageScale)
            win.done()
        }
    }

    Item {
        focus: true
        Keys.onEscapePressed: win.cancel()
    }
}
