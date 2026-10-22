import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Rectangle {
    id: frame

    required property var rect
    property string label
    property var moving: null
    readonly property var shown: moving ?? rect

    signal moved(string rect)

    function drag(dx, dy, resize) {
        const r = rect, fx = dx / parent.width, fy = dy / parent.height;
        if (resize)
            moving = [r[0], r[1], Math.min(1 - r[0], Math.max(0.02, r[2] + fx)), Math.min(1 - r[1], Math.max(0.02, r[3] + fy))];
        else
            moving = [Math.min(1 - r[2], Math.max(0, r[0] + fx)), Math.min(1 - r[3], Math.max(0, r[1] + fy)), r[2], r[3]];
    }

    function release() {
        if (moving)
            moved(moving.map(v => v.toFixed(4)).join(","));
        moving = null;
    }

    x: shown[0] * parent.width
    y: shown[1] * parent.height
    width: shown[2] * parent.width
    height: shown[3] * parent.height
    color: "transparent"
    border.color: Kirigami.Theme.neutralTextColor
    border.width: 2

    QQC2.Label {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.margins: Kirigami.Units.smallSpacing
        text: frame.label
        color: Kirigami.Theme.backgroundColor
        visible: text !== ""

        background: Rectangle {
            color: Kirigami.Theme.neutralTextColor
        }
    }

    Repeater {
        model: [false, true]

        delegate: MouseArea {
            required property bool modelData
            property point start

            anchors.right: parent.right
            anchors.bottom: parent.bottom
            width: modelData ? Kirigami.Units.gridUnit : parent.width
            height: modelData ? Kirigami.Units.gridUnit : parent.height
            cursorShape: modelData ? Qt.SizeFDiagCursor : Qt.SizeAllCursor
            onPressed: mouse => start = mapToItem(frame.parent, mouse.x, mouse.y)
            onPositionChanged: mouse => {
                const at = mapToItem(frame.parent, mouse.x, mouse.y);
                frame.drag(at.x - start.x, at.y - start.y, modelData);
            }
            onReleased: frame.release()

            Rectangle {
                anchors.fill: parent
                visible: parent.modelData
                color: Kirigami.Theme.neutralTextColor
            }
        }
    }
}
