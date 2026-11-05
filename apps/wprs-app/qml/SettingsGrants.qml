import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: permissions

    property var inputs: ({})
    readonly property var grants: inputs.grants ?? {}

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: [App.tr("A scene follows the sound that plays or the pointer, or takes clicks, only after you allow it for that scene, and only while it is shown; clicks also have to be turned on for each screen in its scene settings. The sound is turned into levels on this computer and never recorded, kept or sent; pointer positions and clicks are never kept."), permissions.inputs.capturing ? App.tr("A scene is following the sound now.") : "", permissions.inputs.tracking ? App.tr("A scene is following the pointer now.") : "", Object.keys(permissions.grants).length ? "" : App.tr("No scene has been allowed yet.")].filter(t => t).join("\n")
    }

    Repeater {
        model: Object.keys(permissions.grants)

        delegate: RowLayout {
            id: grant

            required property string modelData

            QQC2.Label {
                text: grant.modelData
            }

            Repeater {
                model: permissions.grants[grant.modelData]

                delegate: QQC2.Button {
                    required property string modelData

                    icon.name: "dialog-cancel"
                    text: App.tr("Stop using %1", Labels.value(modelData))
                    onClicked: App.act("Revoke", [grant.modelData, modelData], "Inputs1")
                }
            }
        }
    }
}
