import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: emitters

    required property var page
    required property string pointer
    readonly property var effects: JSON.parse(page.view && pointer ? page.editor.value(pointer + "/effects") || "null" : "null") ?? []
    readonly property var spots: {
        const out = [];
        effects.forEach((e, k) => {
            if (e.effect !== "fluid" || !Array.isArray(e.params?.emitters))
                return;
            e.params.emitters.forEach((m, i) => {
                out.push({
                    "effect": k,
                    "index": i,
                    "end": "from",
                    "at": m.from,
                    "line": m.to !== undefined,
                    "emitter": m
                });
                if (m.to !== undefined)
                    out.push({
                        "effect": k,
                        "index": i,
                        "end": "to",
                        "at": m.to,
                        "line": true,
                        "emitter": m
                    });
            });
        });
        return out;
    }

    function reach(strength) {
        return Kirigami.Units.gridUnit * (0.8 + Math.min(strength ?? 0, 64) / 16);
    }

    function tip(spot) {
        const a = (spot.emitter.angle ?? -90) * Math.PI / 180, r = reach(spot.emitter.strength);
        return [spot.at[0] * width + r * Math.cos(a), spot.at[1] * height + r * Math.sin(a)];
    }

    function write(spot, change) {
        const list = effects[spot.effect].params.emitters.map(m => Object.assign({}, m));
        change(list[spot.index]);
        page.change(pointer + "/effects/" + spot.effect + "/params/emitters", list, true);
    }

    function place(spot, x, y) {
        write(spot, m => m[spot.end] = [Math.round(x / width * 1000) / 1000, Math.round(y / height * 1000) / 1000]);
    }

    function aim(spot, x, y) {
        const dx = x - spot.at[0] * width, dy = y - spot.at[1] * height;
        const strength = (Math.hypot(dx, dy) / Kirigami.Units.gridUnit - 0.8) * 16;
        write(spot, m => {
            m.angle = Math.round(Math.atan2(dy, dx) * 180 / Math.PI);
            m.strength = Math.round(Math.max(0, Math.min(64, strength)) * 10) / 10;
        });
    }

    onSpotsChanged: lines.requestPaint()
    onWidthChanged: lines.requestPaint()

    Canvas {
        id: lines

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            ctx.lineWidth = 2;
            ctx.strokeStyle = Kirigami.Theme.positiveTextColor;
            emitters.spots.forEach(s => {
                const x = s.at[0] * width, y = s.at[1] * height, end = emitters.tip(s);
                ctx.beginPath();
                if (s.end === "from" && s.line) {
                    ctx.moveTo(x, y);
                    ctx.lineTo(s.emitter.to[0] * width, s.emitter.to[1] * height);
                }
                ctx.moveTo(x, y);
                ctx.lineTo(end[0], end[1]);
                ctx.stroke();
            });
        }
    }

    Repeater {
        model: emitters.spots

        delegate: Rectangle {
            id: knob

            required property var modelData

            x: modelData.at[0] * emitters.width - width / 2
            y: modelData.at[1] * emitters.height - height / 2
            width: Kirigami.Units.gridUnit * 0.8
            height: width
            radius: modelData.line ? 2 : width / 2
            color: Kirigami.Theme.positiveTextColor
            border.color: Kirigami.Theme.backgroundColor

            HoverHandler {
                id: over
            }

            QQC2.ToolTip.visible: over.hovered
            QQC2.ToolTip.text: App.tr("Fluid emitter %1: drag to move it", knob.modelData.index + 1)

            DragHandler {
                target: knob
                onActiveChanged: {
                    if (!active)
                        emitters.place(knob.modelData, knob.x + knob.width / 2, knob.y + knob.height / 2);
                }
            }
        }
    }

    Repeater {
        model: emitters.spots.filter(s => s.end === "from")

        delegate: Rectangle {
            id: grip

            required property var modelData

            x: emitters.tip(modelData)[0] - width / 2
            y: emitters.tip(modelData)[1] - height / 2
            width: Kirigami.Units.gridUnit * 0.6
            height: width
            radius: width / 2
            color: Kirigami.Theme.backgroundColor
            border.color: Kirigami.Theme.positiveTextColor
            border.width: 2

            HoverHandler {
                id: hover
            }

            QQC2.ToolTip.visible: hover.hovered
            QQC2.ToolTip.text: App.tr("Fluid emitter %1: drag to turn it and change its strength", grip.modelData.index + 1)

            DragHandler {
                target: grip
                onActiveChanged: {
                    if (!active)
                        emitters.aim(grip.modelData, grip.x + grip.width / 2, grip.y + grip.height / 2);
                }
            }
        }
    }
}
