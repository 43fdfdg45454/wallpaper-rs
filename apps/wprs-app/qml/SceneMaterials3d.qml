import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    required property string figure
    property var info: null

    function load() {
        info = figure ? JSON.parse(page.editor.rig("", JSON.stringify({
            "materials3d": {
                "layer": figure
            }
        })) || "null") : null;
    }

    function set(name, args) {
        page.run("scene3d", {
            "material": Object.assign({
                "layer": figure,
                "name": name
            }, args)
        }, true);
    }

    visible: (info?.materials ?? []).length > 0
    onFigureChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            panel.load();
        }

        target: panel.page.editor
    }

    Kirigami.Heading {
        level: 5
        text: App.tr("Materials")
    }

    SceneRepeater {
        id: listed

        readonly property var host: panel

        rows: panel.info?.materials ?? []

        delegate: SceneMaterial3d {
            required property int index
            readonly property var modelData: listed.rows[index]

            Layout.fillWidth: true
            panel: listed.host
            material: modelData
        }
    }

    SceneNewMaterial {
        Layout.fillWidth: true
        panel: panel
    }
}
