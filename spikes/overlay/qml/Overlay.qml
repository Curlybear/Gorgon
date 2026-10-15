import QtQuick
import QtQuick.Window
import dev.gorgon.spike

Window {
    id: win

    property var targetScreen
    property Capture capture
    property real originX: 0
    property real originY: 0
    property real imageScale: 1
    signal finished()

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
    title: "Gorgon overlay"

    Component.onCompleted: {
        showFullScreen()
        capture.reportScreen(targetScreen.name, targetScreen.virtualX,
        targetScreen.virtualY, targetScreen.width, targetScreen.height,
        targetScreen.devicePixelRatio)
    }

    Image {
        anchors.fill: parent
        source: win.capture.imageUrl
        sourceClipRect: Qt.rect(win.offsetX, win.offsetY,
                                win.width * win.imageScale, win.height * win.imageScale)
        cache: false
        smooth: false
    }

    SelectionPainter {
        id: painter
        anchors.fill: parent
    }

    Text {
        visible: painter.selection.width > 0
        x: painter.selection.x
        y: Math.max(0, painter.selection.y - height - 4)
        color: "white"
        style: Text.Outline
        text: Math.round(painter.selection.width * win.imageScale) + " × "
              + Math.round(painter.selection.height * win.imageScale)
    }

    MouseArea {
        property point start
        anchors.fill: parent
        cursorShape: Qt.CrossCursor
        onPressed: mouse => {
            start = Qt.point(mouse.x, mouse.y)
            painter.selection = Qt.rect(mouse.x, mouse.y, 0, 0)
        }
        onPositionChanged: mouse => {
            painter.selection = Qt.rect(Math.min(start.x, mouse.x), Math.min(start.y, mouse.y),
                                        Math.abs(mouse.x - start.x), Math.abs(mouse.y - start.y))
        }
        onReleased: {
            const s = painter.selection
            if (s.width < 2 || s.height < 2)
                return
            const path = win.capture.saveCrop(win.offsetX + s.x * win.imageScale,
                                              win.offsetY + s.y * win.imageScale,
                                              s.width * win.imageScale,
                                              s.height * win.imageScale)
            console.log("saved", path)
            win.finished()
        }
    }

    Item {
        focus: true
        Keys.onEscapePressed: win.finished()
    }
}
