import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: rooting

    property var root: null
    property string key: ""

    signal sign(string key, string password)
    signal rotate

    title: App.tr("Root of the repository")
    standardButtons: Kirigami.Dialog.Close
    padding: Kirigami.Units.largeSpacing
    onClosed: password.text = ""

    Kirigami.FormLayout {
        QQC2.Label {
            Kirigami.FormData.label: App.tr("Version:")
            text: rooting.root?.version ?? ""
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Signed:")
            text: rooting.root ? App.tr("%1 of %2 root keys", rooting.root.signers.length, rooting.root.threshold) + (rooting.root.complete ? "" : " · " + App.tr("not complete: clients refuse it until enough root keys sign it")) : ""
        }

        QQC2.TextField {
            Kirigami.FormData.label: App.tr("Anchor for clients:")
            Layout.fillWidth: true
            readOnly: true
            text: rooting.root ? "?" + rooting.root.query : ""
        }

        RowLayout {
            Kirigami.FormData.label: App.tr("Root key:")

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                text: rooting.key || App.tr("A secret key file listed among the root keys")
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

        RowLayout {
            QQC2.Button {
                icon.name: "document-sign"
                text: App.tr("Sign the root")
                enabled: rooting.key !== ""
                onClicked: {
                    rooting.sign(rooting.key, password.text);
                    password.text = "";
                }
            }

            QQC2.Button {
                icon.name: "view-refresh"
                text: App.tr("Rotate the root…")
                enabled: rooting.root?.complete === true
                onClicked: rooting.rotate()
            }
        }
    }

    FileDialog {
        id: keys

        onAccepted: rooting.key = App.path(selectedFile)
    }
}
