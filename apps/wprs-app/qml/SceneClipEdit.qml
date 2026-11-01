import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: edit

    required property var bar
    property bool renaming: false

    function fresh() {
        let n = bar.clips.length + 1;
        while (bar.clips.includes("clip_" + n))
            n++;
        return "clip_" + n;
    }

    QQC2.TextField {
        id: named

        visible: edit.renaming
        text: edit.bar.mesh.clip
        validator: RegularExpressionValidator {
            regularExpression: /[a-z0-9_]{1,32}/
        }
        onEditingFinished: {
            if (edit.renaming && text !== edit.bar.mesh.clip && acceptableInput) {
                edit.bar.clip({
                    "to": text
                });
                edit.bar.mesh.clip = text;
            }
            edit.renaming = false;
        }
    }

    QQC2.ToolButton {
        icon.name: "list-add"
        text: App.tr("Add a clip")
        display: QQC2.AbstractButton.IconOnly
        onClicked: {
            const id = edit.fresh();
            edit.bar.line.page.run("clip", {
                "layer": edit.bar.mesh.layer,
                "id": id,
                "add": true
            });
            edit.bar.mesh.clip = id;
        }
    }

    QQC2.ToolButton {
        icon.name: "edit-rename"
        text: App.tr("Rename the clip")
        display: QQC2.AbstractButton.IconOnly
        onClicked: {
            edit.renaming = true;
            named.forceActiveFocus();
        }
    }

    QQC2.ToolButton {
        icon.name: "edit-delete"
        text: App.tr("Remove the clip")
        display: QQC2.AbstractButton.IconOnly
        onClicked: {
            const gone = edit.bar.mesh.clip;
            edit.bar.mesh.clip = edit.bar.clips.find(c => c !== gone) ?? "";
            edit.bar.line.page.run("clip", {
                "layer": edit.bar.mesh.layer,
                "id": gone,
                "remove": true
            });
        }
    }
}
