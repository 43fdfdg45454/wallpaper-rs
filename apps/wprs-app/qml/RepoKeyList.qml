import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: list

    property var keys: []
    property string label: ""

    RowLayout {
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: list.keys.length > 0 ? list.keys.map(k => k.split("/").pop()).join(", ") : list.label
        }

        QQC2.Button {
            icon.name: "list-add"
            text: App.tr("Add a public key…")
            onClicked: pick.open()
        }

        QQC2.Button {
            icon.name: "edit-clear"
            text: App.tr("Clear")
            display: QQC2.AbstractButton.IconOnly
            enabled: list.keys.length > 0
            onClicked: list.keys = []
        }
    }

    FileDialog {
        id: pick

        nameFilters: [App.tr("Public keys") + " (*.pub)"]
        onAccepted: list.keys = list.keys.concat([App.path(selectedFile)])
    }
}
