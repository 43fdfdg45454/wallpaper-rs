import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: plays

    required property var page
    required property string layerId
    property var info: null
    readonly property var clips: (info?.clips ?? []).map(c => c.id)
    readonly property var bones: (info?.bones ?? []).filter(b => !b.point).map(b => b.name)

    function load() {
        info = layerId ? JSON.parse(page.editor.rig("", JSON.stringify({
            "motion": {
                "layer": layerId
            }
        })) || "null") : null;
    }

    function play(args) {
        page.run("animate", {
            "layer": Object.assign({
                "layer": layerId
            }, args)
        }, true);
    }

    onLayerIdChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            plays.load();
        }

        target: plays.page.editor
    }

    Kirigami.Heading {
        level: 5
        text: App.tr("Animation layers")
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: plays.clips.length === 0
        wrapMode: Text.Wrap
        text: App.tr("Add a clip first; each layer plays one clip and they mix in order.")
    }

    SceneRepeater {
        id: listed

        rows: plays.info?.layers ?? []

        delegate: ColumnLayout {
            id: one

            required property int index
            readonly property var modelData: listed.rows[index]

            Layout.fillWidth: true

            RowLayout {
                QQC2.ComboBox {
                    Layout.fillWidth: true
                    model: plays.clips
                    currentIndex: plays.clips.indexOf(one.modelData.clip)
                    onActivated: index => plays.play({
                            "index": one.modelData.index,
                            "clip": plays.clips[index]
                        })
                }

                Repeater {
                    model: [["enabled", App.tr("On")], ["additive", App.tr("Adds")]]

                    delegate: QQC2.CheckBox {
                        required property var modelData

                        text: modelData[1]
                        checked: one.modelData[modelData[0]] === true
                        onToggled: {
                            const args = {
                                "index": one.modelData.index
                            };
                            args[modelData[0]] = checked;
                            plays.play(args);
                        }
                    }
                }

                QQC2.ToolButton {
                    icon.name: "go-up"
                    enabled: one.modelData.index > 0
                    onClicked: plays.play({
                        "index": one.modelData.index,
                        "to": one.modelData.index - 1
                    })
                }

                QQC2.ToolButton {
                    icon.name: "edit-delete"
                    onClicked: plays.play({
                        "index": one.modelData.index,
                        "remove": true
                    })
                }
            }

            SceneSliders {
                Layout.fillWidth: true
                value: one.modelData
                rows: [["weight", App.tr("Weight"), 0, 1, 0.01], ["rate", App.tr("Speed"), 0, 4, 0.05], ["blend_in", App.tr("Fade in"), 0, 10, 0.1], ["blend_out", App.tr("Fade out"), 0, 10, 0.1]]
                onEdited: (key, value) => {
                    const args = {
                        "index": one.modelData.index
                    };
                    args[key] = value;
                    plays.play(args);
                }
            }

            SceneBonePick {
                Layout.fillWidth: true
                names: plays.bones
                chosen: one.modelData.bones
                label: App.tr("Only these bones")
                onPicked: list => plays.play({
                        "index": one.modelData.index,
                        "bones": list
                    })
            }
        }
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        enabled: plays.clips.length > 0
        displayText: App.tr("Add a layer playing…")
        model: plays.clips
        onActivated: index => plays.play({
                "add": plays.clips[index]
            })
    }
}
