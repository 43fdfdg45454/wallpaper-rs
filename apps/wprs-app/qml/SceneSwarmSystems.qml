import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: systems

    required property var tools
    readonly property var ids: {
        tools.info;
        return Object.keys(JSON.parse(tools.page.editor.value("/scene/particles") || "null") ?? {});
    }

    function rename(from, to) {
        if (to === from || to === "")
            return;
        tools.page.run("particles", {
                "rename": {
                    "system": from,
                    "to": to
                }
            });
        if (tools.page.editor.error === "" && from === tools.system)
            tools.page.editor.select(SceneSwarmKnobs.pointer(to, ""));
    }

    function fresh() {
        let n = 1;
        while (ids.includes("system_" + n))
            n++;
        return "system_" + n;
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Particle systems of the scene")
    }

    Repeater {
        model: systems.ids

        delegate: RowLayout {
            required property string modelData

            Layout.fillWidth: true

            QQC2.TextField {
                Layout.fillWidth: true
                text: modelData
                font.bold: modelData === systems.tools.system
                validator: RegularExpressionValidator {
                    regularExpression: /[a-z0-9_]{1,32}/
                }
                onEditingFinished: systems.rename(modelData, text)
            }

            QQC2.ToolButton {
                icon.name: "document-edit"
                text: App.tr("Edit this system")
                display: QQC2.AbstractButton.IconOnly
                enabled: modelData !== systems.tools.system
                onClicked: systems.tools.page.editor.select(SceneSwarmKnobs.pointer(modelData, ""))
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: App.tr("Remove the system")
                display: QQC2.AbstractButton.IconOnly
                onClicked: systems.tools.page.run("particles", {
                        "delete": {
                            "system": modelData
                        }
                    })
            }
        }
    }

    QQC2.Button {
        icon.name: "list-add"
        text: App.tr("Add a system without a layer")
        onClicked: systems.tools.page.run("particles", {
                "create": {
                    "system": systems.fresh()
                }
            })
    }
}
