import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: life

    required property var window
    readonly property var points: window ?? [0, 0, 1, 1]

    signal edited(var window, bool gesture)

    function put(index, t, gesture) {
        const w = points.slice();
        const lo = index > 0 ? w[index - 1] : 0;
        const hi = index < 3 ? w[index + 1] : 1;
        w[index] = Number(Math.min(hi, Math.max(lo, t)).toFixed(3));
        edited(w, gesture);
    }

    implicitHeight: Kirigami.Units.gridUnit * 3

    SceneHold {
        id: holding

        item: life
    }

    Canvas {
        id: paint

        anchors.fill: parent
        onPaint: {
            const g = getContext("2d"), p = life.points, top = 4, low = height - 4;
            g.reset();
            g.fillStyle = Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.3);
            g.strokeStyle = Kirigami.Theme.highlightColor;
            g.lineWidth = 2;
            g.beginPath();
            g.moveTo(p[0] * width, low);
            g.lineTo(p[1] * width, top);
            g.lineTo(p[2] * width, top);
            g.lineTo(p[3] * width, low);
            g.closePath();
            g.fill();
            g.stroke();
        }
    }

    Connections {
        function onPointsChanged() {
            paint.requestPaint();
        }

        target: life
    }

    Repeater {
        model: 4

        delegate: Rectangle {
            required property int index

            x: life.points[index] * life.width - width / 2
            y: (index === 1 || index === 2 ? 4 : life.height - 4) - height / 2
            width: Kirigami.Units.gridUnit * 0.6
            height: width
            radius: width / 2
            color: Kirigami.Theme.highlightColor

            MouseArea {
                anchors.fill: parent
                anchors.margins: -Kirigami.Units.smallSpacing
                preventStealing: true
                onPressed: holding.hold(true)
                onReleased: holding.hold(false)
                onCanceled: holding.hold(false)
                onPositionChanged: mouse => {
                    if (pressed)
                        life.put(index, mapToItem(life, mouse.x, mouse.y).x / life.width, true);
                }
            }
        }
    }
}
