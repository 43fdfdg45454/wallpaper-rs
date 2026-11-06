import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: lane

    required property var emission
    readonly property var e: emission.emitter
    readonly property real delay: e.delay ?? 0
    readonly property real length: e.duration ?? 0
    readonly property var on: e.periodic?.on ?? null
    readonly property var off: e.periodic?.off ?? null
    readonly property real every: e.burst?.every ?? 0
    readonly property real fitted: Math.max(10, Math.ceil((delay + (length || 0) + (on ? 2 * (on[1] + off[1]) : 0) + every * 2) * 1.2))
    property real held: 0
    readonly property real span: held > 0 ? held : fitted

    function place(t) {
        return t / span * strip.width;
    }

    function when(px) {
        return Math.max(0, Math.round(px / Math.max(1, strip.width) * span * 100) / 100);
    }

    function ranged(pair, v) {
        const width = pair[1] - pair[0];
        return [Math.max(0, Math.round((v - width / 2) * 100) / 100), Math.round((v + width / 2) * 100) / 100];
    }

    onEChanged: art.requestPaint()
    onSpanChanged: art.requestPaint()

    Rectangle {
        id: strip

        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 2.2
        color: Kirigami.Theme.alternateBackgroundColor
        border.color: Kirigami.Theme.disabledTextColor

        Canvas {
            id: art

            anchors.fill: parent
            onWidthChanged: requestPaint()
            onPaint: {
                const ctx = getContext("2d"), h = height, end = lane.length > 0 ? lane.delay + lane.length : lane.span;
                ctx.reset();
                ctx.fillStyle = Kirigami.Theme.highlightColor;
                ctx.globalAlpha = 0.35;
                if (lane.on) {
                    const rows = [[lane.on[0], lane.off[0], 4], [lane.on[1], lane.off[1], h / 2 + 1]];
                    for (const [on, off, top] of rows)
                        for (let s = lane.delay, n = 0; s < end && on + off > 0 && n < 2000; s += on + off, n++)
                            ctx.fillRect(lane.place(s), top, lane.place(Math.min(end, s + on)) - lane.place(s), h / 2 - 5);
                } else {
                    ctx.fillRect(lane.place(lane.delay), 4, lane.place(end) - lane.place(lane.delay), h - 8);
                }
                ctx.globalAlpha = 1;
                ctx.strokeStyle = Kirigami.Theme.positiveTextColor;
                ctx.lineWidth = 2;
                if (lane.e.burst && !lane.e.burst.on)
                    for (let s = lane.delay; s <= end; s += lane.every > 0 ? lane.every : lane.span + 1) {
                        ctx.beginPath();
                        ctx.moveTo(lane.place(s), 0);
                        ctx.lineTo(lane.place(s), h);
                        ctx.stroke();
                    }
            }
        }

        Repeater {
            model: [["delay", lane.delay, App.tr("Start after %1 s", lane.delay)], ["duration", lane.length > 0 ? lane.delay + lane.length : -1, App.tr("Stop after %1 s", lane.length)], ["on", lane.on ? lane.delay + (lane.on[0] + lane.on[1]) / 2 : -1, App.tr("Bursts of emission last %1 to %2 s; the top row shows the shortest, the bottom row the longest", lane.on?.[0] ?? 0, lane.on?.[1] ?? 0)], ["off", lane.on ? lane.delay + (lane.on[0] + lane.on[1]) / 2 + (lane.off[0] + lane.off[1]) / 2 : -1, App.tr("Pauses last %1 to %2 s", lane.off?.[0] ?? 0, lane.off?.[1] ?? 0)], ["every", lane.every > 0 && !lane.e.burst?.on ? lane.delay + lane.every : -1, App.tr("A burst every %1 s", lane.every)]]

            delegate: Rectangle {
                id: knob

                required property var modelData
                property real moved: -1

                visible: modelData[1] >= 0
                x: (moved >= 0 ? moved : lane.place(modelData[1])) - width / 2
                y: modelData[0] === "every" ? 0 : strip.height - height
                width: Kirigami.Units.gridUnit * 0.7
                height: width
                radius: modelData[0] === "delay" || modelData[0] === "duration" ? 0 : width / 2
                color: Kirigami.Theme.highlightColor

                QQC2.ToolTip.text: modelData[2]
                QQC2.ToolTip.visible: hover.hovered || drag.active

                HoverHandler {
                    id: hover
                }

                DragHandler {
                    id: drag

                    target: null
                    onActiveChanged: {
                        lane.emission.tools.page.editor.hold(active);
                        lane.held = active ? lane.fitted : 0;
                        if (!active)
                            knob.moved = -1;
                    }
                    onCentroidChanged: {
                        if (!active)
                            return;
                        knob.moved = Math.max(0, knob.mapToItem(strip, centroid.position.x, 0).x);
                        const at = lane.when(knob.moved), key = knob.modelData[0], start = lane.delay + (lane.on ? (lane.on[0] + lane.on[1]) / 2 : 0);
                        const value = ({
                                "delay": at,
                                "duration": Math.max(0.01, at - lane.delay),
                                "on": ["on", lane.ranged(lane.on ?? [0, 0], Math.max(0.01, at - lane.delay))],
                                "off": ["off", lane.ranged(lane.off ?? [0, 0], Math.max(0.01, at - start))],
                                "every": Math.max(0.01, at - lane.delay)
                            })[key];
                        if (key === "on" || key === "off")
                            lane.emission.put({
                                    "periodic": Object.assign({}, lane.e.periodic, {
                                            [value[0]]: value[1]
                                        })
                                }, true);
                        else if (key === "every")
                            lane.emission.put({
                                    "burst": Object.assign({}, lane.e.burst, {
                                            "every": value
                                        })
                                }, true);
                        else
                            lane.emission.put({
                                    [key]: value
                                }, true);
                    }
                }
            }
        }
    }

    RowLayout {
        Repeater {
            model: [["duration", App.tr("Stop after a while"), lane.length > 0, {
                        "duration": 5
                    }], ["periodic", App.tr("Emit in bursts and pauses"), lane.on !== null, {
                        "periodic": {
                            "on": [1, 1],
                            "off": [1, 1]
                        }
                    }]]

            delegate: QQC2.CheckBox {
                required property var modelData

                text: modelData[1]
                checked: modelData[2]
                onToggled: lane.emission.put(checked ? modelData[3] : {
                        [modelData[0]]: null
                    })
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignRight
            opacity: 0.7
            text: App.tr("%1 s shown", lane.span)
        }
    }
}
