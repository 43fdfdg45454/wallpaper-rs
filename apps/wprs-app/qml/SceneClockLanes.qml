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
    readonly property var clocks: info?.clocks ?? []

    function load() {
        info = JSON.parse(page.editor.rig("", JSON.stringify({
                        "clocks": {
                            "time": page.editor.time
                        }
                    })) || "null");
    }

    function span(clock) {
        return Math.max(clock.length ?? 0, clock.end ?? 0, clock.start + 1, 0.1);
    }

    function trim(clock, key, seconds) {
        const value = Math.round(Math.max(0, Math.min(span(clock), seconds)) * 100) / 100;
        page.change(clock.pointer + "/" + key, value, true);
    }

    spacing: 0
    visible: clocks.length > 0
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            lanes.load();
        }

        function onTimeChanged() {
            lanes.load();
        }

        target: lanes.page.editor
    }

    Repeater {
        model: lanes.clocks

        delegate: RowLayout {
            id: row

            required property var modelData
            readonly property real total: lanes.span(modelData)

            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.8

            QQC2.Label {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                text: App.tr("%1 · own clock", row.modelData.layer)
                elide: Text.ElideRight
            }

            Item {
                id: bar

                Layout.fillWidth: true
                Layout.fillHeight: true

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width
                    height: Kirigami.Units.gridUnit * 0.5
                    radius: height / 2
                    color: Kirigami.Theme.alternateBackgroundColor
                }

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    x: row.modelData.start / row.total * bar.width
                    width: Math.max(2, ((row.modelData.end ?? row.total) - row.modelData.start) / row.total * bar.width)
                    height: Kirigami.Units.gridUnit * 0.5
                    color: Kirigami.Theme.highlightColor
                    opacity: 0.6
                }

                Rectangle {
                    x: Math.min(row.modelData.at, row.total) / row.total * bar.width - 1
                    width: 2
                    height: bar.height
                    color: Kirigami.Theme.negativeTextColor
                }

                Repeater {
                    model: ["start", "end"]

                    delegate: Rectangle {
                        id: grip

                        required property string modelData
                        readonly property real seconds: row.modelData[modelData] ?? row.total

                        x: seconds / row.total * bar.width - width / 2
                        anchors.verticalCenter: parent.verticalCenter
                        width: Kirigami.Units.gridUnit * 0.6
                        height: Kirigami.Units.gridUnit * 1.2
                        radius: 2
                        color: Kirigami.Theme.highlightColor
                        border.color: Kirigami.Theme.backgroundColor

                        HoverHandler {
                            id: over
                        }

                        QQC2.ToolTip.visible: over.hovered || move.active
                        readonly property string held: ((grip.x + grip.width / 2) / bar.width * row.total).toFixed(2)

                        QQC2.ToolTip.text: modelData === "start" ? App.tr("Start %1 s", held) : App.tr("End %1 s", held)

                        DragHandler {
                            id: move

                            target: grip
                            yAxis.enabled: false
                            onActiveChanged: {
                                if (!active)
                                    lanes.trim(row.modelData, grip.modelData, (grip.x + grip.width / 2) / bar.width * row.total);
                            }
                        }
                    }
                }
            }

            QQC2.SpinBox {
                from: 0
                to: 400
                stepSize: 10
                value: Math.round(row.modelData.rate * 100)
                editable: true
                enabled: !row.modelData.keyed
                textFromValue: v => (v / 100).toFixed(2) + "×"
                valueFromText: text => Math.round(Number(text.replace("×", "").replace(",", ".")) * 100)
                onValueModified: lanes.page.change(row.modelData.pointer + "/rate", value / 100)
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: App.tr("Speed of this clock")
            }

            QQC2.CheckBox {
                text: App.tr("Loop")
                checked: row.modelData.loop
                onToggled: lanes.page.change(row.modelData.pointer + "/loop", checked)
            }

            QQC2.CheckBox {
                text: App.tr("Plays")
                checked: row.modelData.playing
                onToggled: lanes.page.change(row.modelData.pointer + "/playing", checked)
            }
        }
    }
}
