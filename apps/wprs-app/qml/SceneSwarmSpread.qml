import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: spread

    required property var tools
    required property var row
    required property var value
    readonly property string key: row[0]
    readonly property var ends: Array.isArray(value) ? [value[0], value[1]] : [row[2], row[3]]
    readonly property real curve: Array.isArray(value) && value.length > 2 ? value[2] : 1
    readonly property var settings: {
        const all = JSON.parse(tools.page.editor.value("/manifest/properties") || "null") ?? {};
        return Object.keys(all).filter(id => all[id].type === "slider");
    }

    function plain(end) {
        return typeof end === "number";
    }

    function write(ends, curve, gesture) {
        const v = ends.slice();
        if (Math.abs(curve - 1) > 1e-3)
            v.push(Number(curve.toFixed(2)));
        spread.tools.edit("start", {
                "ranges": {
                    [spread.key]: v
                }
            }, gesture);
    }

    function number(v) {
        return Number((Math.round(v / spread.row[4]) * spread.row[4]).toFixed(3));
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.preferredWidth: 110
            text: spread.row[1]
        }

        QQC2.RangeSlider {
            id: range

            Layout.fillWidth: true
            from: spread.row[2]
            to: spread.row[3]
            stepSize: spread.row[4]
            enabled: spread.plain(spread.ends[0]) && spread.plain(spread.ends[1])
            first.value: spread.plain(spread.ends[0]) ? spread.ends[0] : from
            second.value: spread.plain(spread.ends[1]) ? spread.ends[1] : to
            first.onMoved: spread.write([spread.number(first.value), spread.ends[1]], spread.curve, true)
            second.onMoved: spread.write([spread.ends[0], spread.number(second.value)], spread.curve, true)
        }

        QQC2.Label {
            text: spread.ends.map(e => spread.plain(e) ? Number(e).toLocaleString(Qt.locale(), "f", spread.row[4] < 1 ? 2 : 0) : App.tr("setting")).join(" – ")
        }
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.preferredWidth: 110
            text: App.tr("Curve")
        }

        Canvas {
            id: plot

            Layout.preferredWidth: 48
            Layout.preferredHeight: 24
            onPaint: {
                const g = getContext("2d");
                g.reset();
                g.strokeStyle = Kirigami.Theme.textColor;
                g.lineWidth = 1.5;
                g.beginPath();
                for (let i = 0; i <= 24; i++) {
                    const x = i / 24;
                    const y = Math.pow(x, spread.curve);
                    if (i === 0)
                        g.moveTo(x * width, height - y * height);
                    else
                        g.lineTo(x * width, height - y * height);
                }
                g.stroke();
            }

            Connections {
                function onCurveChanged() {
                    plot.requestPaint();
                }

                target: spread
            }
        }

        SceneSlider {
            Layout.fillWidth: true
            from: 0.1
            to: 8
            stepSize: 0.05
            value: spread.curve
            onMoved: spread.write(spread.ends, value, true)
            QQC2.ToolTip.text: App.tr("Below 1 picks values near the top end more often, above 1 near the bottom end")
            QQC2.ToolTip.visible: hovered
        }

        Repeater {
            model: [0, 1]

            delegate: QQC2.ComboBox {
                required property int modelData

                model: [modelData === 0 ? App.tr("Low end set here") : App.tr("High end set here")].concat(spread.settings.map(id => App.tr("Follows %1", id)))
                currentIndex: spread.plain(spread.ends[modelData]) ? 0 : 1 + spread.settings.indexOf(spread.ends[modelData]?.prop ?? "")
                onActivated: index => {
                    const ends = spread.ends.slice();
                    ends[modelData] = index === 0 ? spread.row[2 + modelData] : {
                        "prop": spread.settings[index - 1]
                    };
                    spread.write(ends, spread.curve, false);
                }
            }
        }
    }
}
