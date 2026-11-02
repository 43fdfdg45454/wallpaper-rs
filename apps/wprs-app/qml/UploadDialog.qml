import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    property string file: ""

    title: App.tr("Upload to the server")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    onAccepted: App.act("UploadAs", [file, unlisted.checked ? "unlisted" : "public"], "Sync1")

    ColumnLayout {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 24

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: dialog.file.split("/").pop()
        }

        QQC2.RadioButton {
            text: App.tr("Public: in the catalog and its search")
            checked: true
        }

        QQC2.RadioButton {
            id: unlisted

            text: App.tr("Unlisted: only whoever has its link finds it")
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            font: Kirigami.Theme.smallFont
            text: App.tr("An unlisted package is still published and signed: whoever has its link or id can see and install it. You can change this later on your account page.")
        }
    }
}
