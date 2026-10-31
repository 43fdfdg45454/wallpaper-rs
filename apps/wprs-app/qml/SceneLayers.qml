import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    readonly property var layers: page.view.layers ?? []
    readonly property var current: layers.find(layer => layer.pointer === page.editor.selected) ?? null
    readonly property var siblings: current ? layers.filter(layer => layer.pointer.replace(/\/\d+$/, "") === current.pointer.replace(/\/\d+$/, "")) : []
    readonly property var shown: {
        const out = [];
        const visit = parent => layers.filter(layer => layer.pointer.replace(/\/\d+$/, "") === parent).reverse().forEach(layer => {
                out.push(layer);
                visit(layer.pointer + "/layers");
            });
        visit("/scene/layers");
        return out;
    }
    readonly property int index: current ? Number(current.pointer.split("/").pop()) : -1
    readonly property string group: {
        const parent = current ? current.pointer.replace(/\/layers\/\d+$/, "") : "";
        return layers.find(layer => layer.pointer === parent)?.id ?? "";
    }

    readonly property var groups: current ? layers.filter(layer => layer.type === "group" && layer.pointer !== current.pointer && !layer.pointer.startsWith(current.pointer + "/")) : []

    function pick(layer, add) {
        if (!add)
            page.picked = [];
        else if (page.picked.includes(layer.id))
            page.picked = page.picked.filter(id => id !== layer.id);
        else
            page.picked = page.picked.concat([layer.id]);
        page.editor.select(layer.pointer);
    }

    function move(to, into) {
        const args = {
            "id": current.id,
            "to": to
        };
        if (into)
            args.into = into;
        page.run("move_layer", args);
    }

    function icon(type) {
        return ({
                "image": "image-x-generic",
                "video": "video-x-generic",
                "text": "draw-text",
                "shape": "draw-rectangle",
                "solid": "fill-color",
                "spectrum": "view-statistics",
                "particles": "weather-snow",
                "sound": "audio-x-generic",
                "group": "folder",
                "adjustment": "color-management"
            })[type] ?? "layer-visible-on";
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: panel.shown
            clip: true

            delegate: QQC2.ItemDelegate {
                id: row

                required property var modelData

                width: ListView.view.width
                highlighted: modelData.pointer === panel.page.editor.selected || panel.page.picked.includes(modelData.id)
                leftPadding: Kirigami.Units.largeSpacing * (1 + modelData.depth * 2)
                onClicked: panel.pick(modelData, false)

                MouseArea {
                    anchors.fill: parent
                    z: -1
                    onClicked: mouse => panel.pick(row.modelData, (mouse.modifiers & Qt.ControlModifier) !== 0)
                }

                contentItem: RowLayout {
                    Kirigami.Icon {
                        source: panel.icon(row.modelData.type)
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        text: row.modelData.label ?? row.modelData.id
                        elide: Text.ElideRight
                        opacity: row.modelData.hidden ? 0.5 : 1
                    }

                    Repeater {
                        model: [["hide", "hidden", row.modelData.hidden, "view-hidden", App.tr("Hide")], ["lock", "locked", row.modelData.locked, "object-locked", App.tr("Lock")], ["solo", "", row.modelData.solo, "favorite", App.tr("Solo")]]

                        delegate: QQC2.ToolButton {
                            required property var modelData

                            icon.name: modelData[3]
                            text: modelData[4]
                            display: QQC2.AbstractButton.IconOnly
                            checkable: true
                            checked: modelData[2]
                            opacity: checked ? 1 : 0.4
                            onClicked: {
                                const args = {
                                    "id": row.modelData.id
                                };
                                if (modelData[1])
                                    args[modelData[1]] = checked;
                                panel.page.run(modelData[0], modelData[0] === "solo" && !checked ? {} : args);
                            }
                        }
                    }
                }
            }
        }
    }

    Flow {
        Layout.fillWidth: true

        QQC2.ToolButton {
            text: App.tr("Add layer")
            icon.name: "list-add"
            onClicked: kinds.popup()

            QQC2.Menu {
                id: kinds

                Repeater {
                    model: panel.page.view.kinds ?? []

                    delegate: QQC2.MenuItem {
                        required property string modelData

                        enabled: !["image", "video", "sound"].includes(modelData) || (panel.page.view.assets ?? []).some(asset => asset.kind === modelData)
                        text: SceneWords.kind(modelData) + (enabled ? "" : " (" + App.tr("import a file of this kind first") + ")")
                        icon.name: panel.icon(modelData)
                        onTriggered: panel.page.run("add_layer", {
                            "kind": modelData,
                            "name": SceneWords.kind(modelData),
                            "into": panel.current?.type === "group" ? panel.current.id : undefined
                        })
                    }
                }
            }
        }

        Repeater {
            model: [["duplicate_layer", "edit-copy", App.tr("Duplicate")], ["remove_layer", "edit-delete", App.tr("Remove")], ["ungroup", "format-remove-node", App.tr("Ungroup")]]

            delegate: QQC2.ToolButton {
                required property var modelData

                icon.name: modelData[1]
                text: modelData[2]
                display: QQC2.AbstractButton.IconOnly
                enabled: panel.current !== null && (modelData[0] !== "ungroup" || panel.current.type === "group")
                onClicked: panel.page.run(modelData[0], {
                    "id": panel.current.id
                })
            }
        }

        QQC2.ToolButton {
            icon.name: "object-group"
            text: App.tr("Group the layers")
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: App.tr("Group the selected layer and the ones picked with Ctrl")
            QQC2.ToolTip.visible: hovered
            enabled: panel.current !== null
            onClicked: {
                panel.page.run("group", {
                    "ids": [panel.current.id].concat(panel.page.picked.filter(id => id !== panel.current.id))
                });
                panel.page.picked = [];
            }
        }

        QQC2.ToolButton {
            icon.name: "folder-new"
            text: App.tr("Move into a group")
            display: QQC2.AbstractButton.IconOnly
            enabled: panel.groups.length > 0
            onClicked: into.popup()

            QQC2.Menu {
                id: into

                Repeater {
                    model: panel.groups

                    delegate: QQC2.MenuItem {
                        required property var modelData

                        text: modelData.id
                        onTriggered: panel.move(1000, modelData.id)
                    }
                }
            }
        }

        QQC2.ToolButton {
            icon.name: "go-up"
            text: App.tr("Raise")
            display: QQC2.AbstractButton.IconOnly
            enabled: panel.current !== null && panel.index < panel.siblings.length - 1
            onClicked: panel.move(panel.index + 1, panel.group)
        }

        QQC2.ToolButton {
            icon.name: "go-down"
            text: App.tr("Lower")
            display: QQC2.AbstractButton.IconOnly
            enabled: panel.current !== null && panel.index > 0
            onClicked: panel.move(panel.index - 1, panel.group)
        }

        QQC2.ToolButton {
            icon.name: "go-parent-folder"
            text: App.tr("Move out of the group")
            display: QQC2.AbstractButton.IconOnly
            enabled: panel.group !== ""
            onClicked: panel.move(1000)
        }
    }
}
