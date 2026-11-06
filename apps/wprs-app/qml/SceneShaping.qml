import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: shaping

    required property var page
    required property var node
    required property string pointer

    SceneLayerLook {
        Layout.fillWidth: true
        page: shaping.page
        node: shaping.node
    }

    SceneCanvasLook {
        Layout.fillWidth: true
        visible: shaping.pointer === "/scene/canvas"
        page: shaping.page
    }

    SceneSwarmInstance {
        Layout.fillWidth: true
        page: shaping.page
        node: shaping.node
    }

    SceneEffectMask {
        Layout.fillWidth: true
        page: shaping.page
        pointer: shaping.pointer
    }

    ScenePropertyRange {
        Layout.fillWidth: true
        page: shaping.page
        pointer: shaping.pointer
    }
}
