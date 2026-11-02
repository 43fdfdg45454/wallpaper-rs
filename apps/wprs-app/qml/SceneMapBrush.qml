import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

GridLayout {
    id: brush

    required property var page
    required property var node
    readonly property string painting: page.brush?.action === "paint_map" && page.brush.layer === node?.id ? page.brush.map : ""

    function pick(map) {
        page.brush = map ? {
            "action": "paint_map",
            "pointer": node.pointer + "/lighting",
            "layer": node.id,
            "map": map,
            "radius": Math.round(size.value * 1000) / 1000,
            "hardness": Math.round(hard.value * 100) / 100,
            "value": Math.round(gray.value * 100) / 100
        } : null;
    }

    columns: 2

    QQC2.Button {
        text: App.tr("Paint roughness on the preview")
        icon.name: "draw-brush"
        checkable: true
        checked: brush.painting === "roughness"
        onToggled: brush.pick(checked ? "roughness" : "")
    }

    QQC2.Button {
        text: App.tr("Paint glow on the preview")
        icon.name: "draw-brush"
        checkable: true
        checked: brush.painting === "emissive"
        onToggled: brush.pick(checked ? "emissive" : "")
    }

    QQC2.Label {
        visible: brush.painting !== ""
        text: App.tr("Brush size")
    }

    QQC2.Slider {
        id: size

        Layout.fillWidth: true
        visible: brush.painting !== ""
        from: 0.01
        to: 0.5
        value: 0.08
        onMoved: brush.pick(brush.painting)
    }

    QQC2.Label {
        visible: brush.painting !== ""
        text: App.tr("Hardness")
    }

    QQC2.Slider {
        id: hard

        Layout.fillWidth: true
        visible: brush.painting !== ""
        value: 0.5
        onMoved: brush.pick(brush.painting)
    }

    QQC2.Label {
        visible: brush.painting !== ""
        text: App.tr("Gray")
    }

    QQC2.Slider {
        id: gray

        Layout.fillWidth: true
        visible: brush.painting !== ""
        value: 1
        onMoved: brush.pick(brush.painting)
    }
}
