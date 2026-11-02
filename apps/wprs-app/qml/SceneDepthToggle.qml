import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: toggle

    required property var page
    required property var node
    readonly property bool group: node?.type === "group"
    readonly property bool deep: group && JSON.parse(page.editor.value(node.pointer + "/space") || "null") === "3d"
    readonly property bool inside: {
        const up = String(node?.pointer ?? "").replace(/\/layers\/\d+$/, "");
        return up !== String(node?.pointer ?? "") && JSON.parse(page.editor.value(up + "/space") || "null") === "3d";
    }

    visible: node !== null && (group || (inside && !["model", "light", "sound"].includes(node.type)))

    QQC2.CheckBox {
        visible: toggle.group
        text: App.tr("3D group: models, lights and flat layers share depth")
        checked: toggle.deep
        onToggled: toggle.page.run("scene3d", {
            "space": {
                "layer": toggle.node.id,
                "deep": checked
            }
        })
    }

    QQC2.CheckBox {
        visible: !toggle.group
        text: App.tr("Hide behind nearer models")
        checked: toggle.node !== null && toggle.page.editor.value(toggle.node.pointer + "/depth_test") === "true"
        onToggled: toggle.page.run("scene3d", {
            "depth_test": {
                "layer": toggle.node.id,
                "on": checked
            }
        })
    }
}
