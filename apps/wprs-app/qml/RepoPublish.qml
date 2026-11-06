import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: publishing

    property string key: ""
    property alias secret: password.text

    signal publish(string key, string password, int days)

    title: App.tr("Publish the repository")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    padding: Kirigami.Units.largeSpacing
    onAccepted: {
        publish(key, password.text, days.value);
        password.text = "";
    }
    onRejected: password.text = ""

    Kirigami.FormLayout {
        RowLayout {
            Kirigami.FormData.label: App.tr("Online key:")

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                text: publishing.key || App.tr("A secret key file listed among the online keys")
            }

            QQC2.Button {
                icon.name: "document-open"
                text: App.tr("Choose…")
                onClicked: keys.open()
            }
        }

        Kirigami.PasswordField {
            id: password

            Kirigami.FormData.label: App.tr("Password of the key:")
            placeholderText: App.tr("Empty for a key without one")
        }

        QQC2.SpinBox {
            id: days

            Kirigami.FormData.label: App.tr("Days until the index expires:")
            from: 1
            to: 365
            value: 7
        }
    }

    FileDialog {
        id: keys

        onAccepted: publishing.key = App.path(selectedFile)
    }
}
