import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: dialogs

    required property var page
    readonly property alias adder: adder
    readonly property alias linker: linker
    readonly property alias remover: remover
    readonly property alias resetter: resetter
    readonly property alias escrower: escrower
    readonly property alias restorer: restorer

    Kirigami.Dialog {
        id: adder

        title: App.tr("Add server")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: pin.text.trim() === "" ? App.act("AddServer", [url.text.trim()], "Sync1") : App.act("AddPinnedServer", [url.text.trim(), pin.text.trim()], "Sync1")

        ColumnLayout {
            QQC2.TextField {
                id: url

                implicitWidth: Kirigami.Units.gridUnit * 24
                placeholderText: "https://wallpapers.example.org"
            }

            QQC2.TextField {
                id: pin

                Layout.fillWidth: true
                placeholderText: App.tr("Fingerprint of its own certificate, only for a server at home")
            }
        }
    }

    Kirigami.Dialog {
        id: linker

        title: App.tr("Link this computer")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: dialogs.page.link(name.text.trim(), plaintext.checked)

        ColumnLayout {
            QQC2.TextField {
                id: name

                Layout.fillWidth: true
                placeholderText: App.tr("Name of this computer, seen only by your devices")
            }

            QQC2.CheckBox {
                id: plaintext

                text: App.tr("Sync without end-to-end encryption")
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: plaintext.checked
                wrapMode: Text.Wrap
                color: Kirigami.Theme.negativeTextColor
                text: App.tr("The operator of the server will be able to read your library, playlists and settings.")
            }
        }
    }

    Kirigami.Dialog {
        id: remover

        title: App.tr("Remove server")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("RemoveServer", [wipe.checked], "Sync1")

        QQC2.CheckBox {
            id: wipe

            text: App.tr("Also forget the synced copies on this computer")
        }
    }

    Kirigami.Dialog {
        id: resetter

        title: App.tr("Reset synced data")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Reset", [], "Sync1")

        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 24
                wrapMode: Text.Wrap
                text: App.tr("This deletes every synced record on the server and uploads the library, playlists and settings of this computer under a new key. Every other device has to get the key again. Use it only when no device holds the key any more.")
            }
        }
    }

    Kirigami.Dialog {
        id: escrower

        title: App.tr("Recovery passphrase")
        standardButtons: Kirigami.Dialog.Cancel
        onClosed: {
            first.text = "";
            second.text = "";
        }
        customFooterActions: [
            Kirigami.Action {
                text: App.tr("Lock the key with it")
                icon.name: "document-encrypt"
                enabled: first.text.length >= 20 && first.text === second.text
                onTriggered: {
                    App.passphrase("SetEscrow", first.text);
                    escrower.close();
                }
            }
        ]

        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 24
                wrapMode: Text.Wrap
                text: App.tr("The server will keep the sync key locked with this passphrase, so that a new computer can get it when no device is left. Whoever gets a copy of the server's data can try to guess it without limit: use at least 20 characters, several unrelated words that you use nowhere else, and keep them somewhere safe.")
            }

            Kirigami.PasswordField {
                id: first

                Layout.fillWidth: true
                placeholderText: App.tr("Passphrase")
            }

            Kirigami.PasswordField {
                id: second

                Layout.fillWidth: true
                placeholderText: App.tr("Repeat the passphrase")
            }
        }
    }

    Kirigami.Dialog {
        id: restorer

        title: App.tr("Restore the sync key")
        standardButtons: Kirigami.Dialog.Cancel
        onClosed: phrase.text = ""
        customFooterActions: [
            Kirigami.Action {
                text: App.tr("Restore")
                icon.name: "document-decrypt"
                enabled: phrase.text.length >= 20
                onTriggered: {
                    App.passphrase("Restore", phrase.text);
                    restorer.close();
                }
            }
        ]

        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 24
                wrapMode: Text.Wrap
                text: App.tr("Use it only when none of your devices holds the key any more: give the recovery passphrase that you set on one of them.")
            }

            Kirigami.PasswordField {
                id: phrase

                Layout.fillWidth: true
                placeholderText: App.tr("Passphrase")
            }
        }
    }
}
