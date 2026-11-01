import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: pins

    required property var host
    readonly property var page: host.page
    readonly property var mesh: page.mesh
    readonly property string layerId: host.layerId
    readonly property bool active: mesh.mode === "attach" && mesh.rigging === layerId
    readonly property var others: (page.view.layers ?? []).filter(l => l.id !== layerId)
    property var followers: []

    function load() {
        followers = others.map(l => ({
                    "id": l.id,
                    "attach": JSON.parse(page.editor.value(l.pointer + "/attach") || "null")
                })).filter(l => l.attach && l.attach.layer === layerId);
    }

    function follow(kind, on) {
        mesh.inherit = ["position", "rotation", "scale"].filter(k => k === kind ? on : mesh.inherit.includes(k));
    }

    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            pins.load();
        }

        target: pins.page.editor
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    QQC2.Label {
        text: App.tr("Layers that follow this mesh")
        font.bold: true
    }

    Repeater {
        model: pins.followers

        delegate: SceneRigRow {
            required property var modelData

            text: App.tr("%1 follows %2", modelData.id, modelData.attach.bone ?? modelData.attach.point)
            onRemoved: pins.page.run("attach", {
                "layer": modelData.id,
                "detach": true
            })
        }
    }

    QQC2.Button {
        text: App.tr("Attach layers on the preview")
        icon.name: "insert-link"
        checkable: true
        enabled: pins.host.editable && pins.others.length > 0
        checked: pins.active
        onToggled: pins.mesh.use(pins.layerId, checked ? "attach" : "")
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: pins.active
        wrapMode: Text.Wrap
        text: App.tr("Drag a layer onto a bone, or onto the mesh for a new point there; or pick a layer and click the preview.")
    }

    Flow {
        Layout.fillWidth: true
        visible: pins.active
        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: pins.active ? pins.others : []

            delegate: Item {
                id: slot

                required property var modelData

                width: chip.width
                height: chip.height

                QQC2.Button {
                    id: chip

                    readonly property string layerId: slot.modelData.id

                    text: slot.modelData.id
                    checkable: true
                    checked: pins.mesh.attaching === layerId
                    Drag.active: grab.active
                    Drag.keys: ["wprs-layer"]
                    Drag.hotSpot: Qt.point(width / 2, height / 2)
                    onClicked: pins.mesh.attaching = checked ? layerId : ""

                    DragHandler {
                        id: grab

                        onActiveChanged: {
                            if (active)
                                return;
                            chip.Drag.drop();
                            chip.x = 0;
                            chip.y = 0;
                        }
                    }
                }
            }
        }
    }

    RowLayout {
        visible: pins.active

        Repeater {
            model: ["position", "rotation", "scale"]

            delegate: QQC2.CheckBox {
                required property string modelData

                text: SceneWords.value(modelData)
                checked: pins.mesh.inherit.includes(modelData)
                onToggled: pins.follow(modelData, checked)
            }
        }
    }
}
