import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: follow

    required property var page
    required property var node
    readonly property var meshes: (page.view.outside ?? []).filter(l => l.type === "mesh" || l.type === "model").map(l => l.id)
    readonly property var attached: node ? JSON.parse(page.editor.value(node.pointer + "/attach") || "null") : null
    property string mesh: ""
    readonly property var info: mesh !== "" ? JSON.parse(page.editor.rig(mesh, "{}") || "null") : null
    readonly property var spots: (info?.bones ?? []).map(b => ({
                "kind": "bone",
                "name": b.name
            })).concat((info?.points ?? []).filter(p => !p.part).map(p => ({
                "kind": "point",
                "name": p.name
            })))

    function attach(spot) {
        const args = {
            "layer": node.id,
            "to": mesh
        };
        args[spot.kind] = spot.name;
        page.run("attach", args);
    }

    visible: node !== null && meshes.length > 0
    onAttachedChanged: {
        if (attached && meshes.includes(attached.layer))
            mesh = attached.layer;
    }
    onMeshesChanged: {
        if (!meshes.includes(mesh))
            mesh = meshes.length > 0 ? meshes[0] : "";
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Follow a mesh of the scene")
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: follow.attached && follow.meshes.includes(follow.attached.layer) ? App.tr("Every copy follows %1 of %2", follow.attached.bone ?? follow.attached.point, follow.attached.layer) : App.tr("Copies of this template may follow a bone or point of a mesh of the scene")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: follow.meshes
        currentIndex: follow.meshes.indexOf(follow.mesh)
        onActivated: index => follow.mesh = follow.meshes[index]
    }

    QQC2.ComboBox {
        id: spot

        Layout.fillWidth: true
        model: follow.spots.map(s => s.kind === "bone" ? App.tr("Bone %1", s.name) : App.tr("Point %1", s.name))
        enabled: follow.spots.length > 0
    }

    RowLayout {
        QQC2.Button {
            text: App.tr("Follow")
            icon.name: "insert-link"
            enabled: spot.currentIndex >= 0 && spot.currentIndex < follow.spots.length
            onClicked: follow.attach(follow.spots[spot.currentIndex])
        }

        QQC2.Button {
            text: App.tr("Stop following")
            icon.name: "remove-link"
            enabled: follow.attached !== null
            onClicked: follow.page.run("attach", {
                "layer": follow.node.id,
                "detach": true
            })
        }
    }
}
