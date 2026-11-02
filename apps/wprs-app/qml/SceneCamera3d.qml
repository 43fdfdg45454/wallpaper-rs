import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    property var info: null
    readonly property var camera: info?.camera ?? null

    function load() {
        info = JSON.parse(page.editor.rig("", JSON.stringify({
            "stage3d": {
                "time": page.editor.time
            }
        })) || "null");
    }

    function set(args) {
        page.run("scene3d", {
            "camera": args
        }, true);
    }

    function view(args) {
        page.run("scene3d", {
            "view": args
        });
    }

    visible: (info?.stages ?? 0) > 0 || page.editor.value("/scene/camera3d") !== ""
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            panel.load();
        }

        target: panel.page.editor
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("3D camera")
    }

    SceneSliders {
        Layout.fillWidth: true
        value: {
            "fov": panel.camera?.fov ?? 60,
            "roll": 0
        }
        rows: [["fov", App.tr("Field of view"), 1, 179, 1], ["roll", App.tr("Roll"), -180, 180, 1]]
        onEdited: (key, value) => {
            const args = {};
            args[key] = value;
            panel.set(args);
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("From %1 toward %2", (panel.camera?.eye ?? []).join(", "), (panel.camera?.center ?? []).join(", "))
    }

    Flow {
        Layout.fillWidth: true

        QQC2.Button {
            icon.name: "camera-photo"
            text: App.tr("Set the camera from the view")
            onClicked: panel.set({
                "from_view": true
            })
        }

        QQC2.Button {
            icon.name: "visibility"
            text: App.tr("Look through the camera")
            checkable: true
            checked: panel.info?.through ?? true
            onToggled: panel.view({
                "through": checked
            })
        }

        QQC2.Button {
            icon.name: "view-refresh"
            text: App.tr("Back to the camera view")
            onClicked: panel.view({
                "reset": true
            })
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("On the preview: drag with the right button to turn the view, with the middle button or Shift to slide it, and roll the wheel to come closer")
    }
}
