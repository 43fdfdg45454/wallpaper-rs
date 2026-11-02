import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    property var info: null
    property var cost: null

    function ask(q) {
        return JSON.parse(page.editor.rig("", JSON.stringify(q)) || "null");
    }

    function load() {
        if (!visible)
            return;
        info = ask({
            "lights": {
                "time": page.editor.time
            }
        });
        cost = ask({
            "lighting": {
                "time": page.editor.time,
                "quality": SceneLook.quality
            }
        });
    }

    onVisibleChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            panel.load();
        }

        target: panel.page.editor
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Lights")
    }

    Repeater {
        model: panel.info?.lights ?? []

        delegate: RowLayout {
            required property var modelData

            Layout.fillWidth: true

            QQC2.ToolButton {
                Layout.fillWidth: true
                text: modelData.id + " · " + SceneWords.label(modelData.type, "")
                onClicked: panel.page.editor.select(modelData.pointer)
            }

            QQC2.ToolButton {
                icon.name: "media-record"
                text: App.tr("Solo")
                checkable: true
                checked: modelData.solo
                onToggled: panel.page.run("light", {
                    "solo": {
                        "layer": checked ? modelData.id : null
                    }
                })
            }

            QQC2.ToolButton {
                icon.name: "audio-volume-muted"
                text: App.tr("Mute")
                checkable: true
                checked: modelData.muted
                onToggled: panel.page.run("light", {
                    "mute": {
                        "layer": modelData.id,
                        "muted": checked
                    }
                })
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: App.tr("%1 of at most %2 lights", panel.info?.count ?? 0, panel.info?.max ?? 12)
    }

    Repeater {
        model: panel.info?.tiers ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: modelData.left_out > 0 ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
            text: App.tr("%1: %2 of %3 lights, %4 left out", modelData.tier.toUpperCase(), modelData.kept, modelData.cap, modelData.left_out)
        }
    }

    SceneAmbient {
        Layout.fillWidth: true
        page: panel.page
    }

    SceneQuality {
        page: panel.page
        again: App.tr("Restart the preview")
        onPicked: panel.load()
    }

    Repeater {
        model: panel.cost?.levels ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            font.bold: modelData.quality === SceneLook.quality
            text: modelData.baked ? App.tr("%1: lighting worked out once as the scene starts; nothing more each frame", modelData.quality.toUpperCase()) : App.tr("%1: %2 lights over %3 lit layers, %4 pixel lights each frame", modelData.quality.toUpperCase(), modelData.lights, modelData.lit_layers, Math.round(modelData.per_frame))
        }
    }
}
