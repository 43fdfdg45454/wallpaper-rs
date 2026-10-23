import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: timeline

    required property var page
    readonly property real length: page.view.length ?? 10
    readonly property var tracks: page.view.tracks ?? []
    readonly property var easings: page.view.easings ?? ["linear"]
    readonly property var names: [""].concat((page.view.timelines ?? []).map(line => line.id))
    property var chosen: null
    readonly property var picked: chosen ? (tracks.find(track => track.pointer === chosen.pointer)?.keys ?? [])[chosen.index] ?? null : null

    function seek(time, playing) {
        page.clock = time;
        page.editor.seek(time, playing);
    }

    function add() {
        const list = page.view.timelines ?? [];
        let n = list.length + 1;
        while (list.some(line => line.id === "timeline_" + n))
            n++;
        const value = {
            "id": "timeline_" + n,
            "duration": 5,
            "loop": "repeat"
        };
        page.run(list.length ? "add" : "set", {
                "pointer": "/scene/timelines" + (list.length ? "/-" : ""),
                "value": list.length ? value : [value]
            });
    }

    function drive(track, index) {
        const field = track.pointer + "/timeline";
        if (index > 0)
            page.run("set", {
                    "pointer": field,
                    "value": names[index]
                });
        else if (track.timeline)
            page.run("remove", {
                    "pointer": field
                });
    }

    function edit(ease, remove) {
        page.run("keyframe", {
                "pointer": chosen.pointer,
                "time": picked[0],
                "value": picked[1],
                "ease": ease,
                "remove": remove
            });
    }

    spacing: 0

    Timer {
        interval: 50
        repeat: true
        running: timeline.page.editor.playing && timeline.visible
        onTriggered: timeline.page.clock = (timeline.page.clock + interval / 1000) % timeline.length
    }

    QQC2.ToolBar {
        Layout.fillWidth: true

        RowLayout {
            anchors.fill: parent

            QQC2.ToolButton {
                icon.name: timeline.page.editor.playing ? "media-playback-pause" : "media-playback-start"
                text: timeline.page.editor.playing ? App.tr("Pause") : App.tr("Play")
                display: QQC2.AbstractButton.IconOnly
                onClicked: timeline.seek(timeline.page.clock, !timeline.page.editor.playing)
            }

            QQC2.Label {
                text: App.tr("%1 s").arg(timeline.page.clock.toFixed(2))
            }

            QQC2.Slider {
                Layout.fillWidth: true
                from: 0
                to: timeline.length
                value: timeline.page.clock
                onMoved: timeline.seek(value, false)
            }

            QQC2.ComboBox {
                enabled: timeline.picked !== null
                model: timeline.easings.map(ease => SceneWords.easing(ease))
                currentIndex: timeline.picked ? Math.max(0, timeline.easings.indexOf(timeline.picked[2] ?? "linear")) : 0
                onActivated: index => timeline.edit(timeline.easings[index], false)
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: App.tr("Remove the keyframe")
                display: QQC2.AbstractButton.IconOnly
                enabled: timeline.picked !== null
                onClicked: {
                    timeline.edit(undefined, true);
                    timeline.chosen = null;
                }
            }

            QQC2.ToolButton {
                icon.name: "chronometer"
                text: App.tr("Timelines")
                onClicked: lines.popup()

                QQC2.Menu {
                    id: lines

                    Repeater {
                        model: timeline.page.view.timelines ?? []

                        delegate: QQC2.MenuItem {
                            required property var modelData

                            text: modelData.id + " · " + App.tr("%1 s").arg(modelData.duration)
                            onTriggered: timeline.page.editor.select(modelData.pointer)
                        }
                    }

                    QQC2.MenuItem {
                        text: App.tr("Add a timeline")
                        icon.name: "list-add"
                        onTriggered: timeline.add()
                    }
                }
            }
        }
    }

    Kirigami.PlaceholderMessage {
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.margins: Kirigami.Units.largeSpacing
        visible: timeline.tracks.length === 0
        text: App.tr("No keyframes yet: the ◇ button next to a value adds one at the current time.")
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        visible: timeline.tracks.length > 0

        ListView {
            model: timeline.tracks
            clip: true

            delegate: RowLayout {
                id: track

                required property var modelData

                width: ListView.view.width
                height: Kirigami.Units.gridUnit * 1.6

                QQC2.Label {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                    text: (track.modelData.layer ?? "") + " · " + SceneWords.label(track.modelData.key)
                    elide: Text.ElideRight
                }

                QQC2.ComboBox {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    model: timeline.names.map(name => name || App.tr("Scene clock"))
                    currentIndex: Math.max(0, timeline.names.indexOf(track.modelData.timeline ?? ""))
                    onActivated: index => timeline.drive(track.modelData, index)
                }

                Rectangle {
                    id: lane

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: Kirigami.Theme.alternateBackgroundColor

                    Rectangle {
                        x: timeline.page.clock / timeline.length * lane.width
                        width: 1
                        height: parent.height
                        color: Kirigami.Theme.negativeTextColor
                    }

                    Repeater {
                        model: track.modelData.keys

                        delegate: QQC2.Label {
                            required property var modelData
                            required property int index

                            x: modelData[0] / timeline.length * lane.width - width / 2
                            anchors.verticalCenter: parent.verticalCenter
                            text: "◆"
                            color: timeline.chosen?.pointer === track.modelData.pointer && timeline.chosen?.index === index ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor

                            TapHandler {
                                onTapped: {
                                    timeline.chosen = {
                                        "pointer": track.modelData.pointer,
                                        "index": parent.index
                                    };
                                    timeline.seek(parent.modelData[0], false);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
