import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: bar

    required property var stops
    readonly property int most: 8

    signal edited(var stops, bool gesture)

    function sorted(list) {
        return list.slice().sort((a, b) => a[0] - b[0]);
    }

    function moved(index, t, gesture) {
        const list = stops.map(s => s.slice());
        const stop = list[index];
        stop[0] = Number(Math.min(1, Math.max(0, t)).toFixed(3));
        const next = sorted(list);
        edited(next, gesture);
        return next.indexOf(stop);
    }

    function near(t, reach) {
        let best = -1;
        stops.forEach((s, k) => {
                if (Math.abs(s[0] - t) <= reach && (best < 0 || Math.abs(s[0] - t) < Math.abs(stops[best][0] - t)))
                    best = k;
            });
        return best;
    }

    function added(t) {
        if (stops.length >= most)
            return;
        const after = stops.findIndex(s => s[0] > t);
        const near = stops[after < 0 ? stops.length - 1 : after][1] ?? "#ffffff";
        edited(sorted(stops.concat([[Number(t.toFixed(3)), near]])), false);
    }

    function removed(index) {
        edited(stops.filter((_, k) => k !== index), false);
    }

    function recolored(index, hex) {
        const list = stops.map(s => s.slice());
        list[index][1] = hex;
        edited(list, false);
    }

    SceneHold {
        id: holding

        item: bar
    }

    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 2

        Canvas {
            id: paint

            anchors.fill: parent
            anchors.bottomMargin: Kirigami.Units.gridUnit / 2
            onPaint: {
                const g = getContext("2d");
                g.reset();
                const fill = g.createLinearGradient(0, 0, width, 0);
                bar.stops.forEach(s => fill.addColorStop(s[0], s[1].length === 9 ? "#" + s[1].slice(7) + s[1].slice(1, 7) : s[1]));
                g.fillStyle = bar.stops.length > 0 ? fill : Kirigami.Theme.backgroundColor;
                g.fillRect(0, 0, width, height);
            }
        }

        MouseArea {
            property int dragging: -1

            anchors.fill: parent
            onPressed: mouse => {
                dragging = bar.near(mouse.x / width, Kirigami.Units.gridUnit / Math.max(1, width));
                if (dragging < 0)
                    bar.added(mouse.x / width);
                else
                    holding.hold(true);
            }
            onPositionChanged: mouse => {
                if (dragging >= 0)
                    dragging = bar.moved(dragging, mouse.x / width, true);
            }
            onReleased: {
                dragging = -1;
                holding.hold(false);
            }
            onCanceled: {
                dragging = -1;
                holding.hold(false);
            }
        }

        Connections {
            function onStopsChanged() {
                paint.requestPaint();
            }

            target: bar
        }

        SceneRepeater {
            id: listed2

            rows: bar.stops

            delegate: Rectangle {
                readonly property var modelData: listed2.rows[index]
                required property int index

                x: modelData[0] * (parent.width - width)
                y: parent.height - height
                width: Kirigami.Units.gridUnit / 2
                height: Kirigami.Units.gridUnit
                color: modelData[1].slice(0, 7)
                border.color: Kirigami.Theme.textColor
                border.width: 1
            }
        }
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        SceneRepeater {
            id: listed

            rows: bar.stops

            delegate: RowLayout {
                readonly property var modelData: listed.rows[index]
                required property int index

                QQC2.Label {
                    text: Number(modelData[0]).toLocaleString(Qt.locale(), "f", 2)
                }

                SceneColor {
                    hex: modelData[1]
                    onPicked: hex => bar.recolored(index, hex)
                }

                QQC2.ToolButton {
                    icon.name: "list-remove"
                    text: App.tr("Remove the stop")
                    display: QQC2.AbstractButton.IconOnly
                    enabled: bar.stops.length > 1
                    onClicked: bar.removed(index)
                }
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Click the bar to add a stop, at most %1, and drag a stop to move it", bar.most)
    }
}
