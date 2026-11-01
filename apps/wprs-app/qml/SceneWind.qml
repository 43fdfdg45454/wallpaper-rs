import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: wind

    required property var page
    required property real ratio
    readonly property bool shown: page.mesh.mode === "wind"
    property var arrow: ({})
    readonly property point middle: Qt.point(width / 2, height / 2)
    readonly property real angle: (arrow.angle ?? 0) * Math.PI / 180
    readonly property real span: (arrow.strength ?? 0) * ratio

    function load() {
        if (shown)
            arrow = page.mesh.ask("", {
                "wind": true
            }) ?? {};
        tip.x = middle.x + span * Math.cos(angle) - tip.width / 2;
        tip.y = middle.y + span * Math.sin(angle) - tip.height / 2;
        line.requestPaint();
    }

    visible: shown
    onShownChanged: load()
    onWidthChanged: load()

    Connections {
        function onViewChanged() {
            wind.load();
        }

        target: wind.page.editor
    }

    Canvas {
        id: line

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d"), x = tip.x + tip.width / 2, y = tip.y + tip.height / 2;
            const a = Math.atan2(y - wind.middle.y, x - wind.middle.x), head = Kirigami.Units.gridUnit * 0.8;
            ctx.reset();
            ctx.strokeStyle = Kirigami.Theme.highlightColor;
            ctx.lineWidth = 3;
            ctx.beginPath();
            ctx.moveTo(wind.middle.x, wind.middle.y);
            ctx.lineTo(x, y);
            ctx.lineTo(x - head * Math.cos(a - 0.4), y - head * Math.sin(a - 0.4));
            ctx.moveTo(x, y);
            ctx.lineTo(x - head * Math.cos(a + 0.4), y - head * Math.sin(a + 0.4));
            ctx.stroke();
        }
    }

    Rectangle {
        id: tip

        width: Kirigami.Units.gridUnit
        height: width
        radius: width / 2
        color: Kirigami.Theme.highlightColor
        border.color: Kirigami.Theme.backgroundColor

        DragHandler {
            onCentroidChanged: line.requestPaint()
            onActiveChanged: {
                if (active)
                    return;
                const dx = tip.x + tip.width / 2 - wind.middle.x, dy = tip.y + tip.height / 2 - wind.middle.y;
                wind.page.run("wind", {
                    "angle": Math.round(Math.atan2(dy, dx) * 180 / Math.PI),
                    "strength": Math.min(10000, Math.round(Math.hypot(dx, dy) / wind.ratio))
                });
            }
        }
    }
}
