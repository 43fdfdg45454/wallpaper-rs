import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    readonly property var properties: page.view.properties ?? []
    readonly property var templates: ({
            "slider": {
                "min": 0,
                "max": 100,
                "step": 1,
                "default": 50
            },
            "bool": {
                "default": false
            },
            "color": {
                "default": "#ffffff"
            },
            "combo": {
                "options": ["one", "two"],
                "default": "one"
            },
            "text": {
                "default": ""
            },
            "group": {},
            "label": {}
        })

    function add(type) {
        let n = properties.length + 1;
        while (properties.some(p => p.id === type + "_" + n))
            n++;
        const value = Object.assign({
                "type": type,
                "label": SceneWords.label(type),
                "order": n
            }, templates[type]);
        page.run("set", {
                "pointer": "/manifest/properties/" + type + "_" + n,
                "value": value
            });
    }

    function swap(i, j) {
        const all = JSON.parse(page.editor.value("/manifest/properties"));
        const a = properties[i], b = properties[j], same = a.order === b.order;
        all[a.id].order = same ? j : b.order;
        all[b.id].order = same ? i : a.order;
        page.run("set", {
                "pointer": "/manifest/properties",
                "value": all
            });
    }

    QQC2.ItemDelegate {
        Layout.fillWidth: true
        highlighted: panel.page.editor.selected === "/manifest"
        icon.name: "documentinfo"
        text: App.tr("Package details")
        onClicked: panel.page.editor.select("/manifest")

        QQC2.ToolTip.text: (panel.page.view.title ?? "") + " · " + (panel.page.view.version ?? "")
        QQC2.ToolTip.visible: hovered
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: panel.page.view.v2 === false
        text: App.tr("This package uses the first version of the manifest. Upgrade it to give it a rating, credits and texts in several languages; what it cannot tell, such as the rating, the flashes and the origin, stays among the problems until you set it.")
        actions: [
            Kirigami.Action {
                text: App.tr("Upgrade")
                icon.name: "go-up"
                onTriggered: panel.page.run("upgrade")
            }
        ]
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    Kirigami.Heading {
        Layout.fillWidth: true
        level: 4
        text: App.tr("User properties")
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: panel.properties
            clip: true

            delegate: QQC2.ItemDelegate {
                id: row

                required property var modelData
                required property int index

                width: ListView.view.width
                highlighted: panel.page.editor.selected === modelData.pointer
                text: modelData.id + " · " + SceneWords.label(modelData.type)
                onClicked: panel.page.editor.select(modelData.pointer)

                QQC2.ToolButton {
                    anchors.right: down.left
                    icon.name: "go-up"
                    text: App.tr("Move up")
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    enabled: row.index > 0
                    onClicked: panel.swap(row.index, row.index - 1)
                }

                QQC2.ToolButton {
                    id: down

                    anchors.right: remove.left
                    icon.name: "go-down"
                    text: App.tr("Move down")
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    enabled: row.index < panel.properties.length - 1
                    onClicked: panel.swap(row.index, row.index + 1)
                }

                QQC2.ToolButton {
                    id: remove

                    anchors.right: parent.right
                    icon.name: "edit-delete"
                    text: App.tr("Remove")
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    onClicked: panel.page.run("remove", {
                            "pointer": row.modelData.pointer
                        })
                }
            }
        }
    }

    QQC2.ToolButton {
        text: App.tr("Add property")
        icon.name: "list-add"
        onClicked: types.popup()

        QQC2.Menu {
            id: types

            Repeater {
                model: Object.keys(panel.templates)

                delegate: QQC2.MenuItem {
                    required property string modelData

                    text: SceneWords.label(modelData)
                    onTriggered: panel.add(modelData)
                }
            }
        }
    }
}
