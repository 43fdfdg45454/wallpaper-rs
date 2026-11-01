import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

RowLayout {
    id: row

    required property var line
    required property var modelData
    readonly property var keys: modelData.keys
    readonly property real current: {
        const t = line.now, k = keys;
        const i = k.findIndex(x => x.t > t);
        if (k.length === 0)
            return 0;
        if (i <= 0)
            return Number(k[i < 0 ? k.length - 1 : 0].value);
        const a = k[i - 1], b = k[i];
        return a.ease === "step" ? a.value : a.value + (b.value - a.value) * (t - a.t) / Math.max(1e-6, b.t - a.t);
    }

    height: Kirigami.Units.gridUnit * 1.6

    QQC2.Label {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 7
        text: row.modelData.kind === "bone" ? row.modelData.name + " · " + SceneWords.label(row.modelData.channel) : row.modelData.kind === "shape" ? App.tr("Shape %1", row.modelData.name) : App.tr("Order of %1", row.modelData.name)
        elide: Text.ElideRight
        font.bold: row.modelData.kind === "bone" && row.modelData.name === row.line.mesh.bone
    }

    QQC2.Slider {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 5
        visible: row.modelData.kind === "shape"
        from: -1
        to: 1
        value: row.current
        onMoved: row.line.key(row.modelData, {
            "time": row.line.now,
            "weight": Math.round(value * 100) / 100
        })
    }

    QQC2.SpinBox {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 5
        visible: row.modelData.kind === "order"
        from: -1000
        to: 1000
        value: Math.round(row.current)
        editable: true
        onValueModified: row.line.key(row.modelData, {
            "time": row.line.now,
            "value": value
        })
    }

    Rectangle {
        id: lane

        Layout.fillWidth: true
        Layout.fillHeight: true
        color: Kirigami.Theme.alternateBackgroundColor

        TapHandler {
            onTapped: point => row.line.seek(point.position.x / lane.width * row.line.length, false)
        }

        Rectangle {
            x: row.line.now / row.line.length * lane.width
            width: 1
            height: parent.height
            color: Kirigami.Theme.negativeTextColor
        }

        Repeater {
            model: row.keys

            delegate: QQC2.Label {
                id: mark

                required property var modelData
                property real moved: -1
                property bool copying: false

                x: (moved >= 0 ? moved : modelData.t) / row.line.length * lane.width - width / 2
                anchors.verticalCenter: parent.verticalCenter
                text: modelData.ease === "step" ? "■" : "◆"
                color: row.line.chosen && row.line.same(row.line.chosen.row, row.modelData) && Math.abs(row.line.chosen.t - modelData.t) < 1e-3 ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor

                QQC2.ToolTip.visible: mark.moved >= 0
                QQC2.ToolTip.text: mark.copying ? App.tr("Copy to %1 s", mark.moved.toFixed(2)) : App.tr("Move to %1 s", mark.moved.toFixed(2))

                DragHandler {
                    target: null
                    yAxis.enabled: false
                    onActiveChanged: {
                        if (active)
                            mark.copying = (centroid.modifiers & Qt.ControlModifier) !== 0;
                        if (!active && mark.moved >= 0 && Math.abs(mark.moved - mark.modelData.t) >= 1e-3)
                            row.line.key(row.modelData, Object.assign({
                                "time": mark.moved,
                                "from": mark.modelData.t,
                                "copy": mark.copying
                            }, row.line.held(row.modelData, mark.modelData)));
                        if (!active)
                            mark.moved = -1;
                    }
                    onCentroidChanged: {
                        if (active)
                            mark.moved = Math.round(Math.max(0, Math.min(1, mark.mapToItem(lane, centroid.position.x, 0).x / lane.width)) * row.line.length * 100) / 100;
                    }
                }

                TapHandler {
                    onTapped: {
                        row.line.chosen = {
                            "row": row.modelData,
                            "t": mark.modelData.t
                        };
                        if (row.modelData.kind === "bone")
                            row.line.mesh.bone = row.modelData.name;
                        row.line.seek(mark.modelData.t, false);
                    }
                }
            }
        }
    }
}
