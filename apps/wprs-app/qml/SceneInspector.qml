import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: inspector

    required property var page
    readonly property string pointer: page.editor.selected
    readonly property var parts: pointer.split("/").slice(1)
    readonly property var node: (page.view.layers ?? []).find(item => item.pointer === pointer) ?? null
    readonly property string up: {
        const tokens = pointer.split("/");
        const last = tokens.pop();
        if (/^\d+$/.test(last) || ["particles", "properties", "assets", "params"].includes(tokens[tokens.length - 1]))
            tokens.pop();
        return tokens.length > 1 ? tokens.join("/") : "";
    }

    readonly property string list: node === null && /\/\d+$/.test(pointer) ? pointer.replace(/\/\d+$/, "") : ""
    readonly property int index: list ? Number(parts[parts.length - 1]) : -1
    readonly property int count: page.view && list ? page.editor.count(list) : 0

    function shift(by) {
        const to = list + "/" + (index + by);
        page.run("move", {
                "from": pointer,
                "to": to
            });
        page.editor.select(to);
    }

    function addEffect(id) {
        const list = pointer + "/effects";
        if (node.effects > 0)
            page.run("add", {
                    "pointer": list + "/-",
                    "value": {
                        "effect": id
                    }
                });
        else
            page.run("set", {
                    "pointer": list,
                    "value": [{
                            "effect": id
                        }]
                });
    }

    Kirigami.Heading {
        Layout.fillWidth: true
        level: 4
        elide: Text.ElideLeft
        text: inspector.node ? inspector.node.id : inspector.parts.map(part => SceneWords.label(part)).join(" › ")
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.ToolButton {
            icon.name: "go-previous"
            text: App.tr("Up")
            display: QQC2.AbstractButton.IconOnly
            enabled: inspector.up !== ""
            onClicked: inspector.page.editor.select(inspector.up)
        }

        QQC2.ToolButton {
            visible: inspector.list !== ""
            icon.name: "go-up"
            text: App.tr("Move up")
            display: QQC2.AbstractButton.IconOnly
            enabled: inspector.index > 0
            onClicked: inspector.shift(-1)
        }

        QQC2.ToolButton {
            visible: inspector.list !== ""
            icon.name: "go-down"
            text: App.tr("Move down")
            display: QQC2.AbstractButton.IconOnly
            enabled: inspector.index < inspector.count - 1
            onClicked: inspector.shift(1)
        }

        QQC2.ToolButton {
            visible: inspector.node !== null && inspector.node.type !== "sound"
            icon.name: "list-add"
            text: App.tr("Add effect")
            onClicked: effects.popup()

            QQC2.Menu {
                id: effects

                Repeater {
                    model: inspector.page.view.effects ?? []

                    delegate: QQC2.MenuItem {
                        required property string modelData

                        text: SceneWords.effect(modelData)
                        onTriggered: inspector.addEffect(modelData)
                    }
                }
            }
        }

        QQC2.ToolButton {
            visible: inspector.node === null && /\/\d+$/.test(inspector.pointer)
            icon.name: "edit-delete"
            text: App.tr("Remove")
            display: QQC2.AbstractButton.IconOnly
            onClicked: {
                inspector.page.run("remove", {
                        "pointer": inspector.pointer
                    });
                inspector.page.editor.select(inspector.up);
            }
        }

        QQC2.ToolButton {
            visible: inspector.node?.type === "particles"
            icon.name: "weather-snow"
            text: App.tr("Edit the particle system")
            display: QQC2.AbstractButton.IconOnly
            onClicked: inspector.page.editor.select("/scene/particles/" + (inspector.page.fields.find(f => f.key === "system")?.value ?? ""))
        }
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: parent.width

            Repeater {
                model: inspector.page.fields

                delegate: SceneInput {
                    required property var modelData

                    Layout.fillWidth: true
                    page: inspector.page
                    field: modelData
                }
            }
        }
    }
}
