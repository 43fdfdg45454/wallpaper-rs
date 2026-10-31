import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: curve

    required property var input
    readonly property var points: (held ?? (Array.isArray(input.value) ? input.value : [])).map(p => [Number(p[0]) || 0, Number(p[1]) || 0])
    readonly property var range: Array.isArray(input.field.range) ? input.field.range : [0, Math.max(1, peak)]
    property var held: null
    property real peak: 1

    function at(x, y) {
        const t = Math.min(1, Math.max(0, x / plot.width)), v = range[0] + (1 - y / plot.height) * (range[1] - range[0]);
        return [Math.round(t * 1000) / 1000, Math.round(Math.max(range[0], v) * 1000) / 1000];
    }

    function send(points, gesture) {
        input.commit(points.slice().sort((a, b) => a[0] - b[0]), gesture);
    }

    function spot(p) {
        return Qt.point(p[0] * plot.width, (1 - (p[1] - range[0]) / Math.max(1e-6, range[1] - range[0])) * plot.height);
    }

    onPointsChanged: {
        if (!held)
            peak = Math.max(1, ...points.map(p => p[1] * 1.25));
        plot.requestPaint();
    }

    Canvas {
        id: plot

        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 6
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            ctx.strokeStyle = Kirigami.Theme.disabledTextColor;
            ctx.strokeRect(0, 0, width, height);
            ctx.strokeStyle = Kirigami.Theme.highlightColor;
            ctx.lineWidth = 2;
            ctx.beginPath();
            curve.points.slice().sort((a, b) => a[0] - b[0]).forEach((p, i) => {
                const s = curve.spot(p);
                i ? ctx.lineTo(s.x, s.y) : ctx.moveTo(s.x, s.y);
            });
            ctx.stroke();
        }

        TapHandler {
            onTapped: point => curve.send(curve.points.concat([curve.at(point.position.x, point.position.y)]), false)
        }

        Repeater {
            model: curve.points.length

            delegate: Rectangle {
                id: knob

                required property int index
                readonly property point at: curve.spot(curve.points[index] ?? [0, 0])

                x: at.x - width / 2
                y: at.y - height / 2
                width: Kirigami.Units.gridUnit * 0.7
                height: width
                radius: width / 2
                color: Kirigami.Theme.textColor

                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -Kirigami.Units.smallSpacing
                    onPressed: curve.held = curve.points.slice()
                    onPositionChanged: mouse => {
                        const p = mapToItem(plot, mouse.x, mouse.y), points = curve.held.slice();
                        points[knob.index] = curve.at(p.x, p.y);
                        curve.held = points;
                        curve.send(points, true);
                    }
                    onReleased: curve.held = null
                    onDoubleClicked: curve.send(curve.points.filter((_, i) => i !== knob.index), false)
                }
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: App.tr("Click to add a point, drag to move it, double-click to remove it. Across is the life of a particle, up its value.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }
}
