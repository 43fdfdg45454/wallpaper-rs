import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: air

    required property var tools
    property var info: null

    function load() {
        info = tools.lamp ? JSON.parse(tools.page.editor.rig(tools.lamp, JSON.stringify({
            "air": {
                "layer": tools.lamp,
                "time": tools.page.editor.time
            }
        })) || "null") : null;
    }

    function put(args, gesture) {
        tools.page.run("light", {
            "volumetric": Object.assign({
                "layer": tools.lamp
            }, args)
        }, gesture === true);
    }

    function level(row) {
        if (!row.on)
            return App.tr("%1: off", row.quality.toUpperCase());
        return App.tr("%1: %2 of the size, %3 samples", row.quality.toUpperCase(), row.scale === 0.5 ? App.tr("half") : App.tr("a quarter"), row.samples);
    }

    visible: info?.fits === true
    Component.onCompleted: load()

    Connections {
        function onLightChanged() {
            air.load();
        }

        target: air.tools
    }

    QQC2.CheckBox {
        text: App.tr("Light the air")
        checked: air.info?.on === true
        onToggled: air.put(checked ? {} : {
            "off": true
        })
    }

    SceneSliders {
        Layout.fillWidth: true
        visible: air.info?.on === true
        value: {
            "density": air.info?.density ?? 0,
            "samples": air.info?.samples ?? 16
        }
        rows: [["density", App.tr("Density"), 0, 1, 0.01], ["samples", App.tr("Samples"), 4, 64, 1]]
        onEdited: (key, value) => air.put({
                [key]: value
            }, true)
    }

    SceneCurvePlot {
        Layout.fillWidth: true
        visible: air.info?.on === true
        curve: air.info?.curve ?? null
        marks: air.info?.marks ?? []
    }

    Repeater {
        model: air.info?.on ? air.info.levels : []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            font.bold: modelData.quality === SceneLook.quality
            text: air.level(modelData)
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: air.info?.on === true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Phones and older computers light the air at a quarter of the size; the preview shows the level picked below.")
    }
}
