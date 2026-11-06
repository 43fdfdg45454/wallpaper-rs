import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: curve

    required property var timeline
    readonly property var track: timeline.chosen ? timeline.tracks.find(t => t.pointer === timeline.chosen.pointer) ?? null : null
    readonly property var from: timeline.picked
    readonly property var to: track && timeline.chosen ? track.keys[timeline.chosen.index + 1] ?? null : null
    readonly property bool scalar: from !== null && to !== null && typeof from[1] === "number" && typeof to[1] === "number"
    readonly property bool shaped: scalar || (from !== null && to !== null && shapeable(from[1]) && shapeable(to[1]))
    readonly property real v0: scalar ? from[1] : 0
    readonly property real v1: scalar ? to[1] : 1
    property var out: null
    property var back: null
    property string held: ""

    function shapeable(v) {
        const list = v !== null && typeof v === "object" && typeof v.length === "number" && v.length > 0;
        return (list && Array.prototype.every.call(v, n => typeof n === "number")) || (typeof v === "string" && /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(v));
    }

    function third() {
        return [(to[0] - from[0]) / 3, (v1 - v0) / 3];
    }

    function refit() {
        out = shaped ? (from[2]?.out ?? third()) : null;
        back = shaped ? (to[2]?.in ?? third()) : null;
        plot.requestPaint();
    }

    function span() {
        const all = [v0, v1, v0 + out[1], v1 - back[1]];
        const lo = Math.min.apply(null, all), hi = Math.max.apply(null, all), pad = Math.max(1e-3, (hi - lo) * 0.1);
        return [lo - pad, hi + pad];
    }

    function spot(t, v) {
        const y = span();
        return [(t - from[0]) / Math.max(1e-6, to[0] - from[0]) * plot.width, (1 - (v - y[0]) / (y[1] - y[0])) * plot.height];
    }

    function grab(px, py) {
        const y = span(), t = from[0] + px / plot.width * (to[0] - from[0]), v = y[0] + (1 - py / plot.height) * (y[1] - y[0]);
        const clamped = Math.max(from[0], Math.min(to[0], t));
        if (held === "out")
            out = [clamped - from[0], v - v0];
        else
            back = [to[0] - clamped, v1 - v];
        plot.requestPaint();
        send();
    }

    function send() {
        const side = held === "out" ? "out_handle" : "in_handle";
        const args = {
            "pointer": track.pointer,
            "time": held === "out" ? from[0] : to[0]
        };
        args[side] = (held === "out" ? out : back).map(n => Math.round(n * 1000) / 1000);
        timeline.page.run("keyframe", args, true);
    }

    function press(side) {
        held = side;
        timeline.page.editor.hold(true);
    }

    function commit() {
        timeline.page.editor.hold(false);
        held = "";
    }

    title: App.tr("Curve to the next keyframe")
    standardButtons: Kirigami.Dialog.Close
    onOpened: refit()
    onFromChanged: refit()

    ColumnLayout {
        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 20
            wrapMode: Text.Wrap
            text: !curve.shaped ? App.tr("Handles bend the curve between two keyframes of numbers, lists of numbers or colors; pick one that has another after it.") : curve.scalar ? App.tr("Drag the two handles to bend how the number moves between the keyframes.") : App.tr("Drag the two handles to bend how fast the value goes from one keyframe to the next: the height is the share of the way, from 0 to 1.")
        }

        Canvas {
            id: plot

            Layout.preferredWidth: Kirigami.Units.gridUnit * 20
            Layout.preferredHeight: Kirigami.Units.gridUnit * 10
            visible: curve.shaped && curve.out !== null
            onPaint: {
                const ctx = getContext("2d");
                ctx.reset();
                ctx.strokeStyle = Kirigami.Theme.disabledTextColor;
                ctx.strokeRect(0, 0, width, height);
                if (!visible)
                    return;
                const a = curve.spot(curve.from[0], curve.v0), b = curve.spot(curve.to[0], curve.v1);
                const p = curve.spot(curve.from[0] + curve.out[0], curve.v0 + curve.out[1]);
                const q = curve.spot(curve.to[0] - curve.back[0], curve.v1 - curve.back[1]);
                ctx.strokeStyle = Kirigami.Theme.highlightColor;
                ctx.lineWidth = 2;
                ctx.beginPath();
                ctx.moveTo(a[0], a[1]);
                ctx.bezierCurveTo(p[0], p[1], q[0], q[1], b[0], b[1]);
                ctx.stroke();
                ctx.lineWidth = 1;
                ctx.strokeStyle = Kirigami.Theme.textColor;
                [[a, p], [b, q]].forEach(pair => {
                    ctx.beginPath();
                    ctx.moveTo(pair[0][0], pair[0][1]);
                    ctx.lineTo(pair[1][0], pair[1][1]);
                    ctx.stroke();
                    ctx.beginPath();
                    ctx.arc(pair[1][0], pair[1][1], 5, 0, 2 * Math.PI);
                    ctx.fill();
                });
            }

            MouseArea {
                anchors.fill: parent
                onPressed: mouse => {
                    const p = curve.spot(curve.from[0] + curve.out[0], curve.v0 + curve.out[1]);
                    const q = curve.spot(curve.to[0] - curve.back[0], curve.v1 - curve.back[1]);
                    curve.press(Math.hypot(mouse.x - p[0], mouse.y - p[1]) <= Math.hypot(mouse.x - q[0], mouse.y - q[1]) ? "out" : "in");
                }
                onPositionChanged: mouse => curve.grab(mouse.x, mouse.y)
                onReleased: curve.commit()
                onCanceled: curve.commit()
            }
        }

        SceneKeyBase {
            Layout.fillWidth: true
            curve: curve
        }
    }
}
