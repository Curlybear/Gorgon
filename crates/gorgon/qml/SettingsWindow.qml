import QtQuick
import QtQuick.Window
import dev.gorgon

// Everything in ~/.config/gorgon/config.toml, plus the imported uploaders.
Window {
    id: win

    signal closed()

    title: "Gorgon settings"
    color: "#1e1f22"
    width: 640
    height: 760
    minimumWidth: 520
    minimumHeight: 420
    visible: true

    readonly property var uploaders: JSON.parse(settings.uploaders)
    readonly property var fileUploaders: uploaders.filter(u => u.uploads)
    readonly property var shorteners: uploaders.filter(u => u.shortens)

    onClosing: closed()

    SettingsModel { id: settings }

    component Heading: Text {
        color: "#f0f0f0"
        font.pixelSize: 16
        font.bold: true
        topPadding: 14
    }
    component Label: Text {
        color: "#a0a4ab"
        font.pixelSize: 13
        wrapMode: Text.Wrap
    }

    Flickable {
        id: scroll
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: footer.top
        contentHeight: content.implicitHeight + 32
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        Column {
            id: content
            x: 24
            y: 8
            width: scroll.width - 48
            spacing: 10

            Heading { text: "Captures" }

            Label { text: "Save captures in" }
            TextBox {
                width: parent.width
                text: settings.saveDir
                placeholder: "~/Pictures/Gorgon"
                onTextChanged: settings.saveDir = text
            }

            Label { text: "File name (strftime pattern, e.g. %Y-%m-%d_%H-%M-%S)" }
            TextBox {
                id: pattern
                width: parent.width
                text: settings.filenamePattern
                onTextChanged: settings.filenamePattern = text
            }
            Label {
                width: parent.width
                text: "A capture now would be named " + settings.preview(pattern.text)
            }

            Check {
                label: "Copy the image to the clipboard"
                checked: settings.copyImage
                onToggled: c => settings.copyImage = c
            }
            Check {
                label: "Show a notification"
                checked: settings.notify
                onToggled: c => settings.notify = c
            }
            Check {
                label: "Open the editor after every capture"
                detail: "Ctrl+Shift+Print always does"
                checked: settings.openEditor
                onToggled: c => settings.openEditor = c
            }

            Heading { text: "Uploading" }

            Check {
                label: "Upload captures"
                checked: settings.upload
                enabled: win.fileUploaders.length > 0
                onToggled: c => settings.upload = c
            }
            Label { text: "Upload with"; leftPadding: 28 }
            Column {
                leftPadding: 28
                Repeater {
                    model: win.fileUploaders
                    Choice {
                        required property var modelData
                        label: modelData.name
                        enabled: settings.upload
                        selected: settings.imageUploader === modelData.name
                        onPicked: settings.imageUploader = modelData.name
                    }
                }
                Label {
                    visible: win.fileUploaders.length === 0
                    text: "No uploader imported yet (see below)."
                }
            }

            Check {
                label: "Shorten links"
                checked: settings.shorten
                enabled: win.shorteners.length > 0
                onToggled: c => settings.shorten = c
            }
            Label { text: "Shorten with"; leftPadding: 28 }
            Column {
                leftPadding: 28
                Repeater {
                    model: win.shorteners
                    Choice {
                        required property var modelData
                        label: modelData.name
                        enabled: settings.shorten
                        selected: settings.urlShortener === modelData.name
                        onPicked: settings.urlShortener = modelData.name
                    }
                }
                Label {
                    visible: win.shorteners.length === 0
                    text: "No URL shortener imported yet."
                }
            }

            Check {
                label: "Copy the link after uploading"
                detail: "It replaces the copied image"
                checked: settings.copyUrl
                onToggled: c => settings.copyUrl = c
            }

            Heading { text: "Uploaders" }
            Label {
                width: parent.width
                text: "ShareX custom uploader files (.sxcu). XBackBone offers one to download; "
                      + "examples/ in the Gorgon repo has a Kutt template."
            }

            Repeater {
                model: win.uploaders
                Rectangle {
                    required property var modelData
                    width: content.width
                    height: 40
                    radius: 6
                    color: "#2b2d31"

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.name
                        color: "#f0f0f0"
                        font.pixelSize: 14
                    }
                    Text {
                        anchors.right: remove.left
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        text: [modelData.uploads ? "uploads files" : "",
                               modelData.shortens ? "shortens links" : ""].filter(s => s).join(", ")
                        color: "#a0a4ab"
                        font.pixelSize: 12
                    }
                    ToolButton {
                        id: remove
                        anchors.right: parent.right
                        anchors.rightMargin: 4
                        anchors.verticalCenter: parent.verticalCenter
                        label: "Remove"
                        onClicked: settings.removeUploader(modelData.name)
                    }
                }
            }

            Row {
                width: parent.width
                spacing: 8
                TextBox {
                    id: importPath
                    width: parent.width - importButton.width - 8
                    placeholder: "Path to a .sxcu file"
                    onAccepted: importButton.clicked()
                }
                ToolButton {
                    id: importButton
                    anchors.verticalCenter: parent.verticalCenter
                    label: "Import"
                    checked: true
                    enabled: importPath.text.trim() !== ""
                    onClicked: {
                        settings.importFile(importPath.text)
                        if (!settings.messageIsError)
                            importPath.text = ""
                    }
                }
            }

            Rectangle {
                width: parent.width
                height: 64
                radius: 6
                color: drop.containsDrag ? "#26324a" : "transparent"
                border.color: drop.containsDrag ? "#3d6fd9" : "#3a3d44"

                Label {
                    anchors.centerIn: parent
                    text: "…or drop .sxcu files here"
                }
                DropArea {
                    id: drop
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: event => {
                        for (const url of event.urls)
                            settings.importFile(url.toString())
                        event.accept()
                    }
                }
            }

            Heading { text: "Shortcuts" }
            Label {
                width: parent.width
                text: "Ctrl+Print captures a region, Print the screen, Alt+Print a window, "
                      + "Ctrl+Shift+Print a region and opens the editor. "
                      + "Ctrl+Alt+Print records a region to MP4, Ctrl+Alt+Shift+Print to GIF; press it again to stop. Your desktop asks to confirm "
                      + "them the first time and lets you change them in its keyboard settings. "
                      + "Without global shortcuts, bind \"gorgon capture region\" (or screen, window) yourself."
            }
        }
    }

    ScrollMark { flickable: scroll }

    Rectangle {
        id: footer
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 56
        color: "#2b2d31"

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 16
            anchors.right: buttons.left
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: settings.message
            color: settings.messageIsError ? "#ff8a80" : "#7fd68a"
            font.pixelSize: 13
            elide: Text.ElideRight
        }
        Row {
            id: buttons
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            ToolButton {
                label: "Close"
                onClicked: win.close()
            }
            ToolButton {
                label: "Save"
                checked: true
                onClicked: settings.save()
            }
        }
    }

    Item {
        focus: true
        Keys.onEscapePressed: win.close()
    }
}
