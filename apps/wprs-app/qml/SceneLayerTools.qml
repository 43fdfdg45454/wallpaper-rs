import QtQuick
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

Flow {
    id: tools

    required property var owner

    QQC2.ToolButton {
        text: App.tr("Add layer")
        icon.name: "list-add"
        onClicked: kinds.popup()

        QQC2.Menu {
            id: kinds

            Repeater {
                model: tools.owner.page.view.kinds ?? []

                delegate: QQC2.MenuItem {
                    required property string modelData

                    enabled: !["image", "video", "sound"].includes(modelData) || (tools.owner.page.view.assets ?? []).some(asset => asset.kind === modelData)
                    text: SceneWords.kind(modelData) + (enabled ? "" : " (" + App.tr("import a file of this kind first") + ")")
                    icon.name: tools.owner.icon(modelData)
                    onTriggered: tools.owner.page.run("add_layer", {
                        "kind": modelData,
                        "name": SceneWords.kind(modelData),
                        "into": tools.owner.current?.type === "group" ? tools.owner.current.id : undefined
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
            enabled: tools.owner.current !== null && (modelData[0] !== "ungroup" || tools.owner.current.type === "group")
            onClicked: tools.owner.page.run(modelData[0], {
                "id": tools.owner.current.id
            })
        }
    }

    QQC2.ToolButton {
        icon.name: "object-group"
        text: App.tr("Group the layers")
        display: QQC2.AbstractButton.IconOnly
        QQC2.ToolTip.text: App.tr("Group the selected layer and the ones picked with Ctrl")
        QQC2.ToolTip.visible: hovered
        enabled: tools.owner.current !== null
        onClicked: {
            tools.owner.page.run("group", {
                "ids": [tools.owner.current.id].concat(tools.owner.page.picked.filter(id => id !== tools.owner.current.id))
            });
            tools.owner.page.picked = [];
        }
    }

    QQC2.ToolButton {
        icon.name: "folder-new"
        text: App.tr("Move into a group")
        display: QQC2.AbstractButton.IconOnly
        enabled: tools.owner.groups.length > 0
        onClicked: into.popup()

        QQC2.Menu {
            id: into

            Repeater {
                model: tools.owner.groups

                delegate: QQC2.MenuItem {
                    required property var modelData

                    text: modelData.id
                    onTriggered: tools.owner.move(1000, modelData.id)
                }
            }
        }
    }

    QQC2.ToolButton {
        icon.name: "go-up"
        text: App.tr("Raise")
        display: QQC2.AbstractButton.IconOnly
        enabled: tools.owner.current !== null && tools.owner.index < tools.owner.siblings.length - 1
        onClicked: tools.owner.move(tools.owner.index + 1, tools.owner.group)
    }

    QQC2.ToolButton {
        icon.name: "go-down"
        text: App.tr("Lower")
        display: QQC2.AbstractButton.IconOnly
        enabled: tools.owner.current !== null && tools.owner.index > 0
        onClicked: tools.owner.move(tools.owner.index - 1, tools.owner.group)
    }

    QQC2.ToolButton {
        icon.name: "go-parent-folder"
        text: App.tr("Move out of the group")
        display: QQC2.AbstractButton.IconOnly
        enabled: tools.owner.group !== ""
        onClicked: tools.owner.move(1000)
    }
}
