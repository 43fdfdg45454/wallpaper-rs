import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

QQC2.ToolBar {
    required property var timeline

    RowLayout {
        anchors.fill: parent

        QQC2.ToolButton {
            icon.name: timeline.page.editor.playing ? "media-playback-pause" : "media-playback-start"
            text: timeline.page.editor.playing ? App.tr("Pause") : App.tr("Play")
            display: QQC2.AbstractButton.IconOnly
            onClicked: timeline.seek(timeline.page.clock, !timeline.page.editor.playing)
        }

        QQC2.Label {
            text: App.tr("%1 s", timeline.page.clock.toFixed(2))
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

                        text: modelData.id + " · " + App.tr("%1 s", modelData.duration)
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
