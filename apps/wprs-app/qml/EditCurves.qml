import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    property string channel: "rgb"
    property var held: null
    readonly property var curve: (page.values.curves ?? {})[channel] ?? {
        "points": [],
        "samples": []
    }

    function at(x, y) {
        return [x / plot.width, 1 - y / plot.height].map(v => Math.min(1, Math.max(0, v)));
    }

    function send(points) {
        page.run("curve", {
            "channel": channel,
            "points": points.map(p => p.map(v => v.toFixed(4)).join(","))
        });
    }

    onCurveChanged: plot.requestPaint()

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("All channels"), App.tr("Red"), App.tr("Green"), App.tr("Blue")]
        onActivated: index => tool.channel = ["rgb", "r", "g", "b"][index]
    }

    Canvas {
        id: plot

        Layout.fillWidth: true
        Layout.preferredHeight: width
        onPaint: {
            const ctx = getContext("2d"), samples = tool.curve.samples;
            ctx.reset();
            ctx.strokeStyle = Kirigami.Theme.disabledTextColor;
            ctx.strokeRect(0, 0, width, height);
            ctx.strokeStyle = Kirigami.Theme.textColor;
            ctx.lineWidth = 2;
            ctx.beginPath();
            samples.forEach((y, i) => ctx.lineTo(i / Math.max(1, samples.length - 1) * width, (1 - y) * height));
            ctx.stroke();
        }

        MouseArea {
            anchors.fill: parent
            onClicked: mouse => tool.send(tool.curve.points.concat([tool.at(mouse.x, mouse.y)]))
        }

        Repeater {
            model: tool.curve.points.length

            delegate: Rectangle {
                required property int index
                readonly property var point: (tool.held ?? tool.curve.points)[index] ?? [0, 0]

                x: point[0] * plot.width - width / 2
                y: (1 - point[1]) * plot.height - height / 2
                width: Kirigami.Units.gridUnit * 0.8
                height: width
                radius: width / 2
                color: Kirigami.Theme.textColor

                MouseArea {
                    anchors.fill: parent
                    onPressed: tool.held = tool.curve.points.slice()
                    onPositionChanged: mouse => {
                        const spot = mapToItem(plot, mouse.x, mouse.y), points = tool.held.slice();
                        points[parent.index] = tool.at(spot.x, spot.y);
                        tool.held = points;
                        tool.send(points);
                    }
                    onReleased: tool.held = null
                    onDoubleClicked: {
                        if (tool.curve.points.length > 2)
                            tool.send(tool.curve.points.filter((point, i) => i !== parent.index));
                    }
                }
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: App.tr("Click to add a point, drag to move it, double-click to remove it.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }

    QQC2.Button {
        text: App.tr("Reset this curve")
        icon.name: "edit-reset"
        onClicked: tool.send([])
    }
}
