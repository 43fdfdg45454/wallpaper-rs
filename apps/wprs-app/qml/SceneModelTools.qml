import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tools

    required property var page
    required property var node
    readonly property string figure: node?.type === "model" ? node.id : ""
    readonly property var pictures: (page.view.assets ?? []).filter(a => a.kind === "image")
    property var info: null
    property string env: ""

    function load() {
        if (!figure) {
            info = null;
            return;
        }
        const asset = JSON.parse(page.editor.value(node.pointer + "/model") || "null");
        info = JSON.parse(page.editor.rig("", JSON.stringify({
            "model_info": {
                "asset": asset
            }
        })) || "null");
        env = JSON.parse(page.editor.value(node.pointer + "/env") || "null") ?? "";
    }

    visible: figure !== ""
    onFigureChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            tools.load();
        }

        target: tools.page.editor
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("3D model")
    }

    SceneModelInfo {
        Layout.fillWidth: true
        info: tools.info
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Reflects")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("Nothing")].concat(tools.pictures.map(a => a.id))
            currentIndex: Math.max(0, tools.pictures.findIndex(a => a.id === tools.env) + 1)
            onActivated: index => tools.page.run("scene3d", {
                    "model": {
                        "set": {
                            "layer": tools.figure,
                            "env": index === 0 ? "" : tools.pictures[index - 1].id
                        }
                    }
                })
        }

        Image {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            Layout.preferredHeight: Kirigami.Units.gridUnit * 2
            fillMode: Image.PreserveAspectCrop
            visible: tools.env !== ""
            source: {
                const found = tools.pictures.find(a => a.id === tools.env);
                return found ? App.url(tools.page.editor.folder + "/" + found.path) : "";
            }
        }
    }

    QQC2.Button {
        icon.name: "zoom-fit-best"
        text: App.tr("Look at it in the authoring view")
        onClicked: tools.page.run("scene3d", {
            "view": {
                "frame": tools.figure
            }
        })
    }

    SceneModelMotion {
        Layout.fillWidth: true
        page: tools.page
        layerId: tools.figure
    }

    SceneMaterials3d {
        Layout.fillWidth: true
        page: tools.page
        figure: tools.figure
    }
}
