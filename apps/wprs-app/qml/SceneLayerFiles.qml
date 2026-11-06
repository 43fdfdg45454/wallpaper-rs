import QtQuick
import QtQuick.Dialogs
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

QQC2.Menu {
    id: files

    required property var owner
    readonly property var filters: ({
            "image": App.tr("Pictures") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.ktx2)",
            "video": App.tr("Videos") + " (*.webm *.mp4 *.mkv)",
            "sound": App.tr("Sounds") + " (*.ogg *.opus *.flac *.wav *.mp3)",
            "mesh": App.tr("Binary glTF meshes") + " (*.glb)"
        })
    property string kind: "image"
    property FileDialog dialog: FileDialog {
        fileMode: FileDialog.OpenFile
        nameFilters: [files.filters[files.kind]]
        onAccepted: files.add(App.path(selectedFile))
    }

    function add(file) {
        owner.page.run("add_layer", {
            "kind": kind,
            "file": file,
            "into": owner.current?.type === "group" ? owner.current.id : undefined
        });
    }

    title: App.tr("Layer from a file")

    Repeater {
        model: [["image", App.tr("Picture…"), "image-x-generic"], ["video", App.tr("Video…"), "video-x-generic"], ["sound", App.tr("Sound…"), "audio-x-generic"], ["mesh", App.tr("Mesh from a .glb file…"), "draw-polyline"]]

        delegate: QQC2.MenuItem {
            required property var modelData

            text: modelData[1]
            icon.name: modelData[2]
            onTriggered: {
                files.kind = modelData[0];
                files.dialog.open();
            }
        }
    }
}
