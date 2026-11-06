import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: links

    required property var page
    required property var node

    SceneRgb {
        Layout.fillWidth: true
        page: links.page
        node: links.node
    }

    SceneTemplateFollow {
        Layout.fillWidth: true
        page: links.page
        node: links.node
    }
}
