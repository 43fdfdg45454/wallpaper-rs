import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: lanes

    required property var timeline
    readonly property var page: timeline.page
    property var info: null
    readonly property var paths: info?.paths ?? []
    readonly property real span: paths.reduce((most, p) => Math.max(most, p.period ?? 0), Math.max(timeline.length, 0.1))

    function load() {
        info = JSON.parse(page.editor.rig("", JSON.stringify({
                        "paths3d": {
                            "time": page.editor.time
                        }
                    })) || "null");
    }

    function move(path, from, to) {
        page.run("scene3d", {
                "path": {
                    "key": {
                        "path": path,
                        "time": to,
                        "from": from
                    }
                }
            });
    }

    spacing: 0
    visible: paths.length > 0
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            lanes.load();
        }

        target: lanes.page.editor
    }

    SceneRepeater {
        id: listed2

        rows: lanes.paths

        delegate: RowLayout {
            id: row

            required property int index
            readonly property var modelData: listed2.rows[index]

            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.6

            QQC2.Label {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                text: App.tr("Camera path %1", row.modelData.id)
                elide: Text.ElideRight
            }

            Rectangle {
                id: lane

                Layout.fillWidth: true
                Layout.fillHeight: true
                color: Kirigami.Theme.alternateBackgroundColor

                Rectangle {
                    x: lanes.page.clock / lanes.span * lane.width
                    width: 1
                    height: parent.height
                    color: Kirigami.Theme.negativeTextColor
                }

                SceneRepeater {
                    id: listed

                    rows: row.modelData.keys

                    delegate: QQC2.Label {
                        id: key

                        required property int index
                        readonly property var modelData: listed.rows[index]
                        property real moved: -1

                        x: (moved >= 0 ? moved : modelData.t) / lanes.span * lane.width - width / 2
                        anchors.verticalCenter: parent.verticalCenter
                        text: "◆"
                        color: Kirigami.Theme.positiveTextColor
                        QQC2.ToolTip.visible: key.moved >= 0 || hover.hovered
                        QQC2.ToolTip.text: App.tr("%1 s", (key.moved >= 0 ? key.moved : key.modelData.t).toFixed(2))

                        HoverHandler {
                            id: hover
                        }

                        DragHandler {
                            target: null
                            yAxis.enabled: false
                            onActiveChanged: {
                                if (!active && key.moved >= 0 && key.moved !== key.modelData.t)
                                    lanes.move(row.modelData.id, key.modelData.t, key.moved);
                                if (!active)
                                    key.moved = -1;
                            }
                            onCentroidChanged: {
                                if (active)
                                    key.moved = Math.round(Math.max(0, Math.min(1, key.mapToItem(lane, centroid.position.x, 0).x / lane.width)) * lanes.span * 100) / 100;
                            }
                        }

                        TapHandler {
                            onTapped: lanes.timeline.seek(key.modelData.t, false)
                        }
                    }
                }
            }
        }
    }
}
