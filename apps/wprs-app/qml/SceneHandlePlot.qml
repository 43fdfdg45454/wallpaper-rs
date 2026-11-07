import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Canvas {
    id: curve

    required property var from
    required property var to
    readonly property bool scalar: from !== null && to !== null && typeof from[1] === "number" && typeof to[1] === "number"
    readonly property bool shaped: scalar || (from !== null && to !== null && shapeable(from[1]) && shapeable(to[1]))
    readonly property real v0: scalar ? from[1] : 0
    readonly property real v1: scalar ? to[1] : 1
    property var out: null
    property var back: null
    property string held: ""

    signal pressed(string side)
    signal dragged(string side, var pair)
    signal released

    function shapeable(v) {
        const list = v !== null && typeof v === "object" && typeof v.length === "number" && v.length > 0;
        return (list && Array.prototype.every.call(v, n => typeof n === "number")) || (typeof v === "string" && /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(v));
    }

    function third() {
        return [(to[0] - from[0]) / 3, (v1 - v0) / 3];
    }

    function refit() {
        const ready = from !== null && to !== null && shaped;
        out = ready ? (from[2]?.out ?? third()) : null;
        back = ready ? (to[2]?.in ?? third()) : null;
        curve.requestPaint();
    }

    function span() {
        const all = [v0, v1, v0 + out[1], v1 - back[1]];
        const lo = Math.min.apply(null, all), hi = Math.max.apply(null, all), pad = Math.max(1e-3, (hi - lo) * 0.1);
        return [lo - pad, hi + pad];
    }

    function spot(t, v) {
        const y = span();
        return [(t - from[0]) / Math.max(1e-6, to[0] - from[0]) * curve.width, (1 - (v - y[0]) / (y[1] - y[0])) * curve.height];
    }

    function press(side) {
        held = side;
        pressed(side);
    }

    function grab(px, py) {
        const y = span(), t = from[0] + px / curve.width * (to[0] - from[0]), v = y[0] + (1 - py / curve.height) * (y[1] - y[0]);
        const clamped = Math.max(from[0], Math.min(to[0], t));
        if (held === "out")
            out = [clamped - from[0], v - v0];
        else
            back = [to[0] - clamped, v1 - v];
        curve.requestPaint();
        dragged(held, (held === "out" ? out : back).map(n => Math.round(n * 1000) / 1000));
    }

    function commit() {
        released();
        held = "";
    }

    onFromChanged: refit()
    onToChanged: refit()

    Layout.preferredWidth: Kirigami.Units.gridUnit * 20
    Layout.preferredHeight: Kirigami.Units.gridUnit * 10
    visible: shaped && out !== null
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
