import QtQuick
import QtQuick.Window
import dev.gorgon

// Past captures, newest first, with their links.
Window {
    id: win

    property var app
    signal closed()

    title: "Gorgon history"
    color: "#1e1f22"
    width: 820
    height: 620
    minimumWidth: 560
    minimumHeight: 360
    visible: true

    property var items: JSON.parse(history.entries)
    property string filter: ""
    readonly property var shown: filter === "" ? items : items.filter(e =>
        (e.name || "").toLowerCase().includes(filter.toLowerCase())
        || (e.url || "").toLowerCase().includes(filter.toLowerCase()))
    property string toast: ""

    onClosing: closed()
    onActiveChanged: if (active) history.reload()

    HistoryModel { id: history }

    Connections {
        target: win.app
        function onHistoryUpdated() { history.reload() }
    }

    function say(text) {
        toast = text
        toastTimer.restart()
    }

    Timer {
        id: toastTimer
        interval: 2000
        onTriggered: win.toast = ""
    }

    Rectangle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        height: 52
        color: "#2b2d31"

        TextBox {
            id: search
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            width: 280
            placeholder: "Search by name or link"
            onTextChanged: win.filter = text
        }
        Text {
            anchors.left: search.right
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: win.shown.length + (win.shown.length === 1 ? " capture" : " captures")
            color: "#a0a4ab"
            font.pixelSize: 13
        }
        Text {
            anchors.right: parent.right
            anchors.rightMargin: 16
            anchors.verticalCenter: parent.verticalCenter
            text: win.toast
            color: "#7fd68a"
            font.pixelSize: 13
        }
    }

    Text {
        anchors.centerIn: parent
        visible: win.items.length === 0
        horizontalAlignment: Text.AlignHCenter
        text: "No captures yet.\nThey show up here as you take them."
        color: "#a0a4ab"
        font.pixelSize: 15
    }

    ListView {
        id: list
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 12
        clip: true
        spacing: 8
        model: win.shown
        boundsBehavior: Flickable.StopAtBounds

        delegate: Rectangle {
            id: row
            required property var modelData
            readonly property var e: modelData
            property bool confirmDelete: false

            width: list.width
            height: Math.max(112, info.implicitHeight + 20)
            radius: 8
            color: "#2b2d31"

            Rectangle {
                id: thumb
                x: 10
                anchors.verticalCenter: parent.verticalCenter
                width: 150
                height: 92
                radius: 4
                color: "#1e1f22"
                clip: true

                Image {
                    anchors.fill: parent
                    visible: row.e.exists
                    source: row.e.exists ? row.e.fileUrl : ""
                    sourceSize.width: 300
                    sourceSize.height: 184
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    cache: false
                }
                Text {
                    anchors.centerIn: parent
                    visible: !row.e.exists
                    text: "File missing"
                    color: "#6b6f77"
                    font.pixelSize: 12
                }
                MouseArea {
                    anchors.fill: parent
                    enabled: row.e.exists
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Qt.openUrlExternally(row.e.fileUrl)
                }
            }

            Column {
                id: info
                anchors.left: thumb.right
                anchors.leftMargin: 14
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4

                Text {
                    width: parent.width
                    text: row.e.name || ""
                    color: "#f0f0f0"
                    font.pixelSize: 14
                    elide: Text.ElideMiddle
                }
                Text {
                    text: new Date(row.e.time * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat)
                          + (row.e.width ? "   ·   " + row.e.width + " × " + row.e.height : "")
                    color: "#a0a4ab"
                    font.pixelSize: 12
                }
                Text {
                    width: parent.width
                    text: row.e.url ? row.e.url
                        : row.e.error ? "Upload failed: " + row.e.error
                        : "Not uploaded"
                    color: row.e.url ? "#7aa7ff" : row.e.error ? "#ff8a80" : "#6b6f77"
                    font.pixelSize: 13
                    elide: Text.ElideRight
                    MouseArea {
                        anchors.fill: parent
                        enabled: !!row.e.url
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Qt.openUrlExternally(row.e.url)
                    }
                }

                Flow {
                    width: parent.width
                    spacing: 2
                    topPadding: 2

                    ToolButton {
                        visible: !!row.e.url
                        label: "Copy link"
                        checked: true
                        onClicked: {
                            history.copy(row.e.url)
                            win.say("Link copied")
                        }
                    }
                    ToolButton {
                        visible: !!row.e.url
                        label: "Open link"
                        onClicked: Qt.openUrlExternally(row.e.url)
                    }
                    ToolButton {
                        visible: row.e.exists && !row.e.url
                        label: row.e.error ? "Retry upload" : "Upload"
                        onClicked: {
                            history.upload(row.e.id)
                            win.say("Uploading…")
                        }
                    }
                    ToolButton {
                        visible: row.e.exists
                        label: "Edit"
                        onClicked: history.edit(row.e.id)
                    }
                    ToolButton {
                        visible: row.e.exists
                        label: "Show in folder"
                        onClicked: Qt.openUrlExternally(row.e.folderUrl)
                    }
                    ToolButton {
                        visible: !!row.e.deletionUrl
                        label: "Delete online"
                        hint: "Opens the uploader's deletion link"
                        onClicked: Qt.openUrlExternally(row.e.deletionUrl)
                    }
                    ToolButton {
                        label: "Forget"
                        hint: "Remove from history; the file stays"
                        onClicked: history.forget(row.e.id)
                    }
                    ToolButton {
                        visible: row.e.exists
                        label: row.confirmDelete ? "Really delete?" : "Delete file"
                        checked: row.confirmDelete
                        onClicked: {
                            if (row.confirmDelete)
                                history.deleteFile(row.e.id)
                            else
                                row.confirmDelete = true
                        }
                    }
                }
            }
        }
    }

    ScrollMark { flickable: list }

    Item {
        focus: true
        Keys.onEscapePressed: win.close()
        Keys.onPressed: event => {
            if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_F) {
                search.edit()
                event.accepted = true
            }
        }
    }
}
