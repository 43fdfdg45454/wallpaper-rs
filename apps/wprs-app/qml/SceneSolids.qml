import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: solids

    required property var page
    required property var node
    required property string pointer

    SceneModelTools {
        Layout.fillWidth: true
        page: solids.page
        node: solids.node
    }

    SceneDepthToggle {
        Layout.fillWidth: true
        page: solids.page
        node: solids.node
    }

    Loader {
        Layout.fillWidth: true
        active: solids.pointer === "/scene/canvas" || solids.pointer === "/scene/camera3d"
        visible: active

        sourceComponent: ColumnLayout {
            SceneCamera3d {
                Layout.fillWidth: true
                page: solids.page
            }

            SceneTones {
                Layout.fillWidth: true
                visible: solids.pointer === "/scene/canvas"
                page: solids.page
            }
        }
    }
}
