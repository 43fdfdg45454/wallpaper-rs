import QtQuick

Item {
    id: nav

    required property var stage
    property var last: null

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.RightButton | Qt.MiddleButton
        onPressed: mouse => nav.last = [mouse.x, mouse.y]
        onReleased: nav.last = null
        onPositionChanged: mouse => {
            if (!nav.last)
                return;
            const d = [mouse.x - nav.last[0], mouse.y - nav.last[1]];
            nav.last = [mouse.x, mouse.y];
            if (mouse.buttons & Qt.MiddleButton || mouse.modifiers & Qt.ShiftModifier)
                nav.stage.view({
                    "pan": [d[0] / nav.stage.ratio, d[1] / nav.stage.ratio]
                });
            else
                nav.stage.view({
                    "orbit": [-d[0] * 0.4, d[1] * 0.4]
                });
        }
        onWheel: wheel => nav.stage.view({
                "zoom": wheel.angleDelta.y > 0 ? 1.1 : 1 / 1.1
            })
    }
}
