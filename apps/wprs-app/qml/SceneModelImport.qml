import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    required property var page
    required property string into
    property string file: ""
    property var info: null

    function pick(path) {
        file = path;
        info = JSON.parse(page.editor.rig("", JSON.stringify({
            "model_info": {
                "file": path
            }
        })) || "null");
    }

    function name() {
        const base = file.split("/").pop().replace(/\.glb$/i, "").toLowerCase().replace(/[^a-z0-9_]+/g, "_").replace(/^_+|_+$/g, "");
        const taken = (page.view.assets ?? []).map(a => a.id).concat((page.view.layers ?? []).map(l => l.id));
        let id = base || "model", n = 1;
        while (taken.includes(id))
            id = (base || "model") + "_" + (++n);
        return id;
    }

    title: App.tr("3D model from a file")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    onOpened: files.open()
    onAccepted: {
        if (!file || info?.error)
            return;
        const id = name();
        page.run("import", {
            "file": file,
            "id": id
        });
        page.run("scene3d", {
            "model": {
                "add": {
                    "asset": id,
                    "id": id + "_layer",
                    "into": into || undefined
                }
            }
        });
    }

    ColumnLayout {
        QQC2.Button {
            icon.name: "document-open"
            text: dialog.file ? dialog.file.split("/").pop() : App.tr("Pick a binary glTF file")
            onClicked: files.open()
        }

        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 22
            wrapMode: Text.Wrap
            visible: dialog.info?.error !== undefined
            color: Kirigami.Theme.negativeTextColor
            text: dialog.info?.error ?? ""
        }

        SceneModelInfo {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 22
            info: dialog.info
        }

        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 22
            wrapMode: Text.Wrap
            font: Kirigami.Theme.smallFont
            text: App.tr("FBX and OBJ files open in Blender and leave it as binary glTF (.glb); the guide in the documentation tells how.")
        }
    }

    FileDialog {
        id: files

        fileMode: FileDialog.OpenFile
        nameFilters: [App.tr("Binary glTF models") + " (*.glb)"]
        onAccepted: dialog.pick(App.path(selectedFile))
    }
}
