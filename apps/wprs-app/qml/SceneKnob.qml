import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Rectangle {
    id: knob

    required property var at
    required property real ratio
    property string label
    property string tip
    property bool square
    property color tint: Kirigami.Theme.highlightColor
    property var grabbed: null

    signal moved(var to, bool done)

    visible: Array.isArray(at)
    x: (at?.[0] ?? 0) * ratio - width / 2
    y: (at?.[1] ?? 0) * ratio - height / 2
    width: Kirigami.Units.gridUnit * 0.8
    height: width
    radius: square ? 2 : width / 2
    color: drag.active ? Kirigami.Theme.positiveTextColor : tint
    border.color: Kirigami.Theme.backgroundColor
    z: 6

    QQC2.Label {
        anchors.left: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: knob.label
        style: Text.Outline
        styleColor: Kirigami.Theme.backgroundColor
    }

    QQC2.ToolTip.visible: hover.hovered && knob.tip !== ""
    QQC2.ToolTip.text: knob.tip

    HoverHandler {
        id: hover
    }

    DragHandler {
        id: drag

        target: null
        onActiveChanged: {
            if (active) {
                knob.grabbed = knob.at.slice();
                return;
            }
            if (knob.grabbed)
                knob.moved([knob.grabbed[0] + translation.x / knob.ratio, knob.grabbed[1] + translation.y / knob.ratio], true);
            knob.grabbed = null;
        }
        onTranslationChanged: {
            if (active && knob.grabbed)
                knob.moved([knob.grabbed[0] + translation.x / knob.ratio, knob.grabbed[1] + translation.y / knob.ratio], false);
        }
    }
}
