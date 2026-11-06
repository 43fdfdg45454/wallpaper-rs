import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: made

    required property var panel
    readonly property var page: panel.page
    readonly property var packages: panel.info?.packages ?? []
    property var checks: ({})

    function check() {
        const out = {};
        packages.forEach(name => out[name] = JSON.parse(page.editor.rig("", JSON.stringify({
                "material_check": {
                    "name": name
                }
            })) || "null"));
        checks = out;
    }

    onPackagesChanged: check()
    Component.onCompleted: check()

    RowLayout {
        QQC2.TextField {
            id: name

            Layout.fillWidth: true
            placeholderText: App.tr("Name of a new material shader")
            validator: RegularExpressionValidator {
                regularExpression: /[a-z0-9_]{1,32}/
            }
        }

        QQC2.Button {
            icon.name: "document-new"
            text: App.tr("New material shader")
            enabled: name.acceptableInput
            onClicked: {
                made.page.run("scene3d", {
                    "new_material": {
                        "name": name.text
                    }
                });
                name.text = "";
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: made.packages.length > 0
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Material shaders write fn material(s: Surface) -> Surface in effects/NAME.wgsl of the package; they read frame and param(0u) to param(7u).")
    }

    SceneRepeater {
        id: listed

        rows: made.packages

        delegate: Kirigami.InlineMessage {
            required property int index
            readonly property var modelData: listed.rows[index]

            readonly property var found: made.checks[modelData] ?? null

            Layout.fillWidth: true
            visible: true
            type: found?.ok ? Kirigami.MessageType.Positive : Kirigami.MessageType.Error
            text: found?.ok ? App.tr("%1 builds", modelData) : found?.line ? App.tr("%1, line %2, column %3: %4", modelData, found.line, found.column, found.error ?? "") : App.tr("%1: %2", modelData, found?.error ?? "")
            actions: [
                Kirigami.Action {
                    text: App.tr("Check again")
                    icon.name: "view-refresh"
                    onTriggered: made.check()
                }
            ]
        }
    }
}
