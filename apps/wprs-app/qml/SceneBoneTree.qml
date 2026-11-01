import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: boneTree

    required property var host
    readonly property var rows: ordered(host.bones)

    function ordered(bones) {
        const out = [];
        const visit = (parent, depth) => bones.filter(b => (b.parent ?? null) === parent).forEach(b => {
                out.push(Object.assign({
                    "depth": depth
                }, b));
                visit(b.name, depth + 1);
            });
        visit(null, 0);
        return out.concat(bones.filter(b => !out.some(o => o.name === b.name)).map(b => Object.assign({
                "depth": 0
            }, b)));
    }

    function hang(name, parent) {
        if (name !== parent)
            host.bone({
                "name": name,
                "parent": parent
            });
    }

    QQC2.Label {
        text: App.tr("Bones")
        font.bold: true
    }

    Repeater {
        model: boneTree.rows

        delegate: SceneBoneRow {
            required property var modelData

            Layout.fillWidth: true
            tree: boneTree
            bone: modelData
        }
    }

    DropArea {
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 1.4
        visible: boneTree.host.editable && boneTree.rows.length > 0
        keys: ["bone"]
        onDropped: drop => boneTree.hang(drop.source.bone.name, "")

        QQC2.Label {
            anchors.fill: parent
            verticalAlignment: Text.AlignVCenter
            opacity: parent.containsDrag ? 1 : 0.6
            text: App.tr("Drop a bone here to hang it from no other bone")
            elide: Text.ElideRight
        }
    }

    RowLayout {
        visible: boneTree.host.editable

        QQC2.TextField {
            id: named

            Layout.fillWidth: true
            placeholderText: App.tr("Name of a new bone")
        }

        QQC2.ComboBox {
            id: under

            model: [App.tr("No parent")].concat(boneTree.host.bones.map(b => b.name))
            currentIndex: Math.max(0, boneTree.host.bones.findIndex(b => b.name === boneTree.host.page.mesh.bone) + 1)
        }

        QQC2.Button {
            text: App.tr("Add")
            enabled: named.text.trim() !== ""
            onClicked: {
                boneTree.host.bone({
                    "name": named.text.trim(),
                    "parent": under.currentIndex > 0 ? boneTree.host.bones[under.currentIndex - 1].name : null
                });
                boneTree.host.page.mesh.bone = named.text.trim();
                named.text = "";
            }
        }
    }
}
