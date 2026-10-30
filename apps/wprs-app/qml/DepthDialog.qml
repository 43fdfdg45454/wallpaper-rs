import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    required property var maps
    readonly property var missing: JSON.parse(maps.assets || "[]").filter(asset => !asset.installed)

    title: App.tr("Download the depth model?")
    standardButtons: Kirigami.Dialog.Cancel
    customFooterActions: [
        Kirigami.Action {
            text: App.tr("Download")
            icon.name: "download"
            enabled: dialog.maps.allowed && dialog.missing.length > 0
            onTriggered: {
                dialog.maps.download();
                dialog.close();
            }
        },
        Kirigami.Action {
            text: App.tr("Install from a file…")
            icon.name: "document-open"
            onTriggered: files.open()
        }
    ]

    ColumnLayout {
        QQC2.Label {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 28
            text: App.tr("Depth maps are estimated on this computer by a model that is not included. The picture never leaves it; these files are downloaded once, checked against their SHA-256 and kept in your data folder:")
            wrapMode: Text.Wrap
        }

        Repeater {
            model: dialog.missing

            delegate: QQC2.Label {
                required property var modelData

                Layout.fillWidth: true
                text: App.tr("%1: %2 from %3, license %4", modelData.title, modelData.size, modelData.hosts, modelData.license)
                wrapMode: Text.Wrap
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: !dialog.maps.allowed
            text: App.tr("Downloads are turned off in the settings. Install files fetched by other means instead.")
            wrapMode: Text.Wrap
        }
    }

    FileDialog {
        id: files

        onAccepted: {
            dialog.maps.install(selectedFile.toString());
            dialog.close();
        }
    }
}
