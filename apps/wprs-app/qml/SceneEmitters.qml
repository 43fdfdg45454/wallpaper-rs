import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: emitters

    required property var page
    required property string pointer
    property var live: null
    readonly property var effects: JSON.parse(page.view && pointer ? page.editor.value(pointer + "/effects") || "null" : "null") ?? []
    readonly property var lists: effects.map((e, k) => {
        const v = e.effect === "fluid" ? e.params?.emitters : null;
        if (v?.map && v.prop !== undefined)
            return Array.isArray(v.map[entry(k, v)]) ? v.map[entry(k, v)] : null;
        if (Array.isArray(v) || !v?.keys || page.editor.time < 0)
            return Array.isArray(v) ? v : null;
        const field = JSON.parse(page.editor.inspect(pointer + "/effects/" + k) || "[]").find(f => f.key === "emitters");
        return Array.isArray(field?.shown) ? field.shown : null;
    })
    readonly property var spots: {
        const out = [];
        lists.forEach((list, k) => {
            (list ?? []).forEach((m, i) => {
                ["from", "to"].filter(end => m[end] !== undefined).forEach(end => out.push({
                            "effect": k,
                            "index": i,
                            "end": end,
                            "at": m[end],
                            "line": m.to !== undefined,
                            "emitter": m
                        }));
            });
        });
        return out;
    }

    function entry(k, v) {
        const keys = Object.keys(v.map ?? {}), picked = page.emitterEntry[pointer + "/effects/" + k];
        const now = JSON.parse(page.editor.value("/manifest/properties/" + SceneFluid.escaped(v.prop) + "/default") || "null");
        const current = now === null ? "null" : String(now);
        return keys.includes(picked) ? picked : keys.includes(current) ? current : keys[0] ?? "";
    }

    function target(k) {
        const v = effects[k]?.params?.emitters, at = pointer + "/effects/" + k + "/params/emitters";
        return v?.map && v.prop !== undefined ? at + "/map/" + SceneFluid.escaped(entry(k, v)) : at;
    }

    function drawn(spot) {
        const m = spot.emitter, l = live && live.effect === spot.effect && live.index === spot.index ? live : null;
        const at = end => l?.end === end ? [l.x, l.y] : [m[end][0] * width, m[end][1] * height];
        const from = at("from"), to = m.to === undefined ? null : at("to");
        if (l?.end === "tip")
            return Object.assign({
                "from": from,
                "to": to,
                "tip": [l.x, l.y]
            }, SceneFluid.aimed(from, l.x, l.y));
        const a = (m.angle ?? -90) * Math.PI / 180, r = SceneFluid.reach(m.strength);
        return {
            "from": from,
            "to": to,
            "tip": [from[0] + r * Math.cos(a), from[1] + r * Math.sin(a)],
            "angle": m.angle ?? -90,
            "strength": m.strength ?? 0
        };
    }

    function follow(spot, end, item, active) {
        if (!active)
            return;
        live = {
            "effect": spot.effect,
            "index": spot.index,
            "end": end,
            "x": item.x + item.width / 2,
            "y": item.y + item.height / 2
        };
        lines.requestPaint();
    }

    function write(spot, change) {
        live = null;
        const list = lists[spot.effect].map(m => Object.assign({}, m));
        change(list[spot.index]);
        page.change(target(spot.effect), list, true);
    }

    function place(spot, x, y) {
        write(spot, m => m[spot.end] = [Math.round(x / width * 1000) / 1000, Math.round(y / height * 1000) / 1000]);
    }

    function aim(spot, x, y) {
        write(spot, m => Object.assign(m, SceneFluid.aimed(drawn(spot).from, x, y)));
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
            emitters.spots.filter(s => s.end === "from").forEach(s => {
                const d = emitters.drawn(s);
                ctx.beginPath();
                if (d.to) {
                    ctx.moveTo(d.from[0], d.from[1]);
                    ctx.lineTo(d.to[0], d.to[1]);
                }
                ctx.moveTo(d.from[0], d.from[1]);
                ctx.lineTo(d.tip[0], d.tip[1]);
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
            onXChanged: emitters.follow(modelData, modelData.end, knob, move.active)
            onYChanged: emitters.follow(modelData, modelData.end, knob, move.active)

            HoverHandler {
                id: over
            }

            QQC2.ToolTip.visible: over.hovered
            QQC2.ToolTip.text: App.tr("Fluid emitter %1: drag to move it", knob.modelData.index + 1)

            DragHandler {
                id: move

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
            readonly property var shape: emitters.drawn(modelData)

            x: shape.tip[0] - width / 2
            y: shape.tip[1] - height / 2
            width: Kirigami.Units.gridUnit * 0.6
            height: width
            radius: width / 2
            color: Kirigami.Theme.backgroundColor
            border.color: Kirigami.Theme.positiveTextColor
            border.width: 2
            onXChanged: emitters.follow(modelData, "tip", grip, turn.active)
            onYChanged: emitters.follow(modelData, "tip", grip, turn.active)

            HoverHandler {
                id: hover
            }

            QQC2.ToolTip.visible: hover.hovered || turn.active
            QQC2.ToolTip.text: turn.active ? App.tr("Direction %1°, strength %2", grip.shape.angle, grip.shape.strength) : App.tr("Fluid emitter %1: drag to turn it and change its strength", grip.modelData.index + 1)

            DragHandler {
                id: turn

                target: grip
                onActiveChanged: {
                    if (!active)
                        emitters.aim(grip.modelData, grip.x + grip.width / 2, grip.y + grip.height / 2);
                }
            }
        }
    }
}
