import QtQuick

// A thin bar along a Flickable's right edge showing where you are.
Rectangle {
    property Flickable flickable

    visible: flickable.contentHeight > flickable.height
    x: flickable.x + flickable.width - width - 2
    y: flickable.y + flickable.visibleArea.yPosition * flickable.height
    width: 4
    height: Math.max(24, flickable.visibleArea.heightRatio * flickable.height)
    radius: 2
    color: "#6b6f77"
    opacity: flickable.moving ? 0.9 : 0.4
}
