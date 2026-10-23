import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: tools

    required property var page
    property var row: ({})
    property var scripts: []
    readonly property var groups: (page.view.properties ?? []).filter(p => p.type === "group" && p.id !== row.id)
    readonly property string named: scripts.length ? App.tr("These scripts name it and are not rewritten: %1").arg(scripts.join(", ")) : ""

    function show(item, data) {
        row = data;
        scripts = JSON.parse(page.editor.scripts(data.id) || "[]");
        menu.popup(item);
    }

    function move(data, to, group) {
        const args = {
            "id": data.id,
            "to": to
        };
        if (group !== undefined)
            args.group = group;
        page.run("move_property", args);
    }

    function under(pointer) {
        return (page.editor.selected + "/").startsWith(pointer + "/");
    }

    function rename(to) {
        const selected = under(row.pointer) ? page.editor.selected : "";
        page.run("rename_property", {
                "id": row.id,
                "to": to
            });
        if (selected && page.editor.error === "")
            page.editor.select("/manifest/properties/" + to + selected.slice(row.pointer.length));
    }

    function remove() {
        const selected = under(row.pointer);
        page.run("remove_property", {
                "id": row.id
            });
        if (selected && page.editor.error === "")
            page.editor.select("/manifest");
    }

    QQC2.Menu {
        id: menu

        QQC2.MenuItem {
            icon.name: "edit-rename"
            text: App.tr("Rename…")
            onTriggered: renaming.open()
        }

        QQC2.Menu {
            title: App.tr("Move to group")
            enabled: tools.row.type !== "group"

            QQC2.MenuItem {
                text: App.tr("No group")
                enabled: tools.row.group !== null
                onTriggered: tools.move(tools.row, ((tools.page.view.properties ?? []).find(p => p.id === tools.row.group)?.index ?? 998) + 1, "")
            }

            Repeater {
                model: tools.groups

                delegate: QQC2.MenuItem {
                    required property var modelData

                    text: modelData.label
                    enabled: tools.row.group !== modelData.id
                    onTriggered: tools.move(tools.row, 1000, modelData.id)
                }
            }
        }

        QQC2.MenuItem {
            icon.name: "edit-delete"
            text: App.tr("Remove")
            onTriggered: tools.scripts.length ? removing.open() : tools.remove()
        }
    }

    Kirigami.PromptDialog {
        id: renaming

        title: App.tr("Rename the property")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onOpened: {
            name.text = tools.row.id;
            name.forceActiveFocus();
            name.selectAll();
        }
        onAccepted: tools.rename(name.text.trim())

        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                wrapMode: Text.Wrap
                text: [App.tr("Bindings, triggers, groups, conditions and presets follow the new id; scripts are not rewritten."), tools.named].filter(text => text).join("\n")
            }

            QQC2.TextField {
                id: name

                Layout.fillWidth: true
                onAccepted: renaming.accept()
            }
        }
    }

    Kirigami.PromptDialog {
        id: removing

        title: App.tr("Remove the property")
        subtitle: tools.named
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: tools.remove()
    }
}
