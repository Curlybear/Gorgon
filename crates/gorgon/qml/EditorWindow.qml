import QtQuick
import QtQuick.Window
import dev.gorgon

// Annotate a capture, then Done copies and saves it.
Window {
    id: win

    signal closed()

    title: "Gorgon editor"
    color: "#1e1f22"
    visible: false
    // Every tool fits on one line.
    minimumWidth: tools.implicitWidth + actions.implicitWidth + 40
    minimumHeight: 360

    readonly property var tools: [
        { id: "select", label: "Select", key: "V" },
        { id: "rect", label: "▭", key: "R", hint: "Rectangle" },
        { id: "ellipse", label: "◯", key: "E", hint: "Ellipse" },
        { id: "arrow", label: "➜", key: "A", hint: "Arrow" },
        { id: "line", label: "╱", key: "L", hint: "Line" },
        { id: "pen", label: "✎", key: "P", hint: "Pen" },
        { id: "text", label: "T", key: "T", hint: "Text" },
        { id: "highlight", label: "▮", key: "H", hint: "Highlighter" },
        { id: "blur", label: "Blur", key: "B" },
        { id: "pixelate", label: "Pixelate", key: "X" },
        { id: "crop", label: "Crop", key: "C" }
    ]
    readonly property var colors: ["#e53935", "#fb8c00", "#ffe030", "#43a047", "#1e88e5",
                                   "#8e24aa", "#000000", "#ffffff"]

    function done() {
        commitText()
        canvas.finish()
        close()
    }

    function commitText() {
        if (textInput.visible) {
            canvas.addText(textInput.x, textInput.y, textInput.text)
            textInput.visible = false
        }
    }

    onClosing: closed()
    // Show the capture at its real size. When it is too big, maximize for a
    // capture as wide as the screen, otherwise shrink the window to the
    // largest size that fits, keeping the capture's shape.
    function showSized() {
        const margin = 2 * canvas.anchors.margins
        const chromeW = margin
        const chromeH = toolbar.height + zoombar.height + margin
        const imgW = canvas.imageWidth / Screen.devicePixelRatio
        const imgH = canvas.imageHeight / Screen.devicePixelRatio
        const maxW = Screen.desktopAvailableWidth * 0.95
        const maxH = Screen.desktopAvailableHeight * 0.95
        const s = Math.min(1, (maxW - chromeW) / imgW, (maxH - chromeH) / imgH)
        if (s < 1 && imgW * s > (maxW - chromeW) * 0.9) {
            showMaximized()
        } else {
            width = Math.max(imgW * s + chromeW, minimumWidth)
            height = Math.max(imgH * s + chromeH, minimumHeight)
            show()
        }
        requestActivate()
    }

    Component.onCompleted: {
        if (canvas.load())
            showSized()
        else
            close()
    }

    Rectangle {
        id: toolbar
        anchors.left: parent.left
        anchors.right: parent.right
        height: 48
        color: "#2b2d31"
        z: 2

        Row {
            id: tools
            anchors.verticalCenter: parent.verticalCenter
            anchors.left: parent.left
            anchors.leftMargin: 8
            spacing: 2

            Repeater {
                model: win.tools
                ToolButton {
                    required property var modelData
                    label: modelData.label
                    hint: (modelData.hint || modelData.label) + " (" + modelData.key + ")"
                    checked: canvas.tool === modelData.id
                    onClicked: canvas.tool = modelData.id
                }
            }

            Item { width: 12; height: 32 }

            Repeater {
                model: win.colors
                Item {
                    required property string modelData
                    width: 28
                    height: 32

                Rectangle {
                    anchors.centerIn: parent
                    width: 22
                    height: 22
                    radius: 11
                    color: parent.modelData
                    border.width: canvas.color === parent.modelData ? 3 : 1
                    border.color: canvas.color === parent.modelData ? "#ffffff" : "#55585e"
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: canvas.color = parent.modelData
                }
                }
            }

            Item { width: 12; height: 32 }

            Repeater {
                model: [2, 4, 8]
                ToolButton {
                    required property int modelData
                    label: modelData === 2 ? "Thin" : (modelData === 4 ? "Medium" : "Thick")
                    checked: canvas.strokeWidth === modelData
                    onClicked: {
                        canvas.strokeWidth = modelData
                        canvas.fontSize = modelData * 7
                    }
                }
            }

            Item { width: 12; height: 32 }

            ToolButton {
                label: "↶"
                hint: "Undo (Ctrl+Z)"
                enabled: canvas.canUndo
                onClicked: canvas.undo()
            }
            ToolButton {
                label: "↷"
                hint: "Redo (Ctrl+Shift+Z)"
                enabled: canvas.canRedo
                onClicked: canvas.redo()
            }
            ToolButton {
                label: "Delete"
                hint: "Delete selected (Del)"
                enabled: canvas.hasSelection
                onClicked: canvas.deleteSelected()
            }
        }

        Row {
            id: actions
            anchors.verticalCenter: parent.verticalCenter
            anchors.right: parent.right
            anchors.rightMargin: 8
            spacing: 6

            ToolButton {
                label: "Cancel"
                hint: "Discard (Esc)"
                onClicked: win.close()
            }
            ToolButton {
                label: "Done"
                hint: "Copy and save (Enter)"
                checked: true
                onClicked: win.done()
            }
        }
    }

    EditorCanvas {
        id: canvas
        pixelRatio: Screen.devicePixelRatio
        anchors.top: toolbar.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: zoombar.top
        anchors.margins: 12
        anchors.topMargin: 12
        onWidthChanged: relayout()
        onHeightChanged: relayout()
        onTextRequested: (x, y) => {
            win.commitText()
            textInput.x = x
            textInput.y = y
            textInput.text = ""
            textInput.visible = true
            textInput.forceActiveFocus()
        }

        MouseArea {
            id: pointer
            // Middle-button drag scrolls a zoomed-in capture.
            property point panFrom
            property bool panning: false

            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
            cursorShape: panning ? Qt.ClosedHandCursor
                       : canvas.tool === "select" ? Qt.ArrowCursor
                       : canvas.tool === "text" ? Qt.IBeamCursor : Qt.CrossCursor
            onPressed: event => {
                if (event.button === Qt.MiddleButton) {
                    win.commitText()
                    panning = true
                    panFrom = Qt.point(event.x, event.y)
                    return
                }
                if (canvas.tool !== "text")
                    win.commitText()
                canvas.press(event.x, event.y)
                if (canvas.tool !== "text")
                    keys.forceActiveFocus()
            }
            onPositionChanged: event => {
                if (panning) {
                    canvas.panBy(event.x - panFrom.x, event.y - panFrom.y)
                    panFrom = Qt.point(event.x, event.y)
                } else {
                    canvas.moveTo(event.x, event.y)
                }
            }
            onReleased: event => {
                if (panning)
                    panning = false
                else
                    canvas.release(event.x, event.y)
            }
            // Ctrl+wheel zooms at the pointer; the wheel scrolls (Shift for sideways).
            onWheel: event => {
                win.commitText()
                const dy = event.angleDelta.y
                if (event.modifiers & Qt.ControlModifier) {
                    if (dy !== 0)
                        canvas.zoomBy(Math.pow(1.0015, dy), event.x, event.y)
                } else if (event.pixelDelta.x !== 0 || event.pixelDelta.y !== 0) {
                    canvas.panBy(event.pixelDelta.x, event.pixelDelta.y)
                } else if (event.modifiers & Qt.ShiftModifier) {
                    canvas.panBy(dy / 2, 0)
                } else {
                    canvas.panBy(event.angleDelta.x / 2, dy / 2)
                }
            }
        }

        TextInput {
            id: textInput
            visible: false
            color: canvas.color
            // The canvas may be scaled down; match the size the text will have.
            font.pixelSize: Math.max(8, canvas.fontSize * canvas.zoom / 100 / Math.max(0.1, canvas.pixelRatio))
            selectByMouse: true
            Keys.onReturnPressed: event => {
                if (event.modifiers & Qt.ShiftModifier) {
                    insert(cursorPosition, "\n")
                } else {
                    win.commitText()
                    keys.forceActiveFocus()
                }
            }
            Keys.onEscapePressed: {
                visible = false
                keys.forceActiveFocus()
            }

            Rectangle {
                anchors.fill: parent
                anchors.margins: -3
                z: -1
                color: "transparent"
                border.color: "#40a0ff"
            }
        }
    }

    // Zoom readout and controls.
    Rectangle {
        id: zoombar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 40
        color: "#2b2d31"
        z: 2

        Text {
            anchors.verticalCenter: parent.verticalCenter
            anchors.left: parent.left
            anchors.leftMargin: 12
            text: canvas.imageWidth + " × " + canvas.imageHeight
            color: "#a0a4ab"
            font.pixelSize: 13
        }

        Row {
            anchors.verticalCenter: parent.verticalCenter
            anchors.right: parent.right
            anchors.rightMargin: 8
            spacing: 2

            ToolButton {
                label: "Fit"
                hint: "Fit to window (Ctrl+0)"
                checked: canvas.fitted
                onClicked: canvas.zoomFit()
            }
            ToolButton {
                label: "−"
                hint: "Zoom out (Ctrl+−)"
                onClicked: canvas.zoomStep(false)
            }
            Rectangle {
                width: 64
                height: 28
                anchors.verticalCenter: parent.verticalCenter
                radius: 6
                color: "#1e1f22"
                border.color: zoomInput.activeFocus ? "#3d6fd9" : "#3a3d44"

                TextInput {
                    id: zoomInput
                    anchors.fill: parent
                    anchors.margins: 4
                    horizontalAlignment: TextInput.AlignHCenter
                    verticalAlignment: TextInput.AlignVCenter
                    color: "#f0f0f0"
                    font.pixelSize: 13
                    selectByMouse: true
                    validator: RegularExpressionValidator { regularExpression: /[0-9]{0,4}%?/ }
                    text: Math.round(canvas.zoom) + "%"
                    onActiveFocusChanged: if (activeFocus) selectAll()
                    function apply() {
                        const n = parseFloat(text)
                        if (n > 0)
                            canvas.setZoomPercent(n)
                        text = Qt.binding(() => Math.round(canvas.zoom) + "%")
                        keys.forceActiveFocus()
                    }
                    Keys.onReturnPressed: apply()
                    Keys.onEnterPressed: apply()
                    Keys.onEscapePressed: {
                        text = Qt.binding(() => Math.round(canvas.zoom) + "%")
                        keys.forceActiveFocus()
                    }
                }
            }
            ToolButton {
                label: "+"
                hint: "Zoom in (Ctrl++)"
                onClicked: canvas.zoomStep(true)
            }
            ToolButton {
                label: "100%"
                hint: "Actual pixels (Ctrl+1)"
                onClicked: canvas.setZoomPercent(100)
            }
        }
    }

    // Keyboard shortcuts, active whenever the text box isn't being typed in.
    Item {
        id: keys
        focus: true
        Keys.onPressed: event => {
            const ctrl = event.modifiers & Qt.ControlModifier
            const shift = event.modifiers & Qt.ShiftModifier
            if (ctrl && event.key === Qt.Key_Z) {
                shift ? canvas.redo() : canvas.undo()
            } else if (ctrl && event.key === Qt.Key_Y) {
                canvas.redo()
            } else if (ctrl && event.key === Qt.Key_0) {
                canvas.zoomFit()
            } else if (ctrl && event.key === Qt.Key_1) {
                canvas.setZoomPercent(100)
            } else if (ctrl && (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal)) {
                canvas.zoomStep(true)
            } else if (ctrl && event.key === Qt.Key_Minus) {
                canvas.zoomStep(false)
            } else if (event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace) {
                canvas.deleteSelected()
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                win.done()
            } else if (event.key === Qt.Key_Escape) {
                win.close()
            } else if (!ctrl) {
                const tool = win.tools.find(t => t.key === event.text.toUpperCase())
                if (!tool)
                    return
                canvas.tool = tool.id
            } else {
                return
            }
            event.accepted = true
        }
    }
}
