import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

GridLayout {
    id: ambient

    required property var page
    property var shown: null

    function load() {
        shown = JSON.parse(page.editor.rig("", JSON.stringify({
            "hemisphere": {}
        })) || "null");
    }

    function hex(name) {
        const v = JSON.parse(page.editor.value("/scene/canvas/" + name) || "null");
        return typeof v === "string" ? v : "#000000";
    }

    columns: 3
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            ambient.load();
        }

        target: ambient.page.editor
    }

    Image {
        Layout.rowSpan: 2
        source: ambient.shown?.sphere ?? ""
        cache: false
    }

    QQC2.Label {
        text: App.tr("Light around")
    }

    SceneColor {
        alpha: false
        hex: ambient.hex("ambient")
        onPicked: hex => ambient.page.run("ambient", {
            "ambient": hex
        })
    }

    QQC2.Label {
        text: App.tr("Light from above")
    }

    SceneColor {
        alpha: false
        hex: ambient.hex("sky")
        onPicked: hex => ambient.page.run("ambient", {
            "sky": hex
        })
    }
}
