import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

Repeater {
    id: grants

    property var given: ({})

    model: Object.keys(given)

    delegate: RowLayout {
        id: grant

        required property string modelData

        QQC2.Label {
            text: grant.modelData
        }

        Repeater {
            model: grants.given[grant.modelData]

            delegate: QQC2.Button {
                required property string modelData

                icon.name: "dialog-cancel"
                text: App.tr("Stop using %1", Labels.value(modelData))
                onClicked: App.act("Revoke", [grant.modelData, modelData], "Inputs1")
            }
        }
    }
}
