import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: dialogs

    required property var page
    property var device: ({})
    readonly property alias adder: adder
    readonly property alias linker: linker
    readonly property alias approver: approver
    readonly property alias remover: remover

    function share(target) {
        device = target;
        sharer.open();
    }

    Kirigami.Dialog {
        id: adder

        title: App.tr("Add server")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("AddServer", [url.text.trim()], "Sync1")

        QQC2.TextField {
            id: url

            implicitWidth: Kirigami.Units.gridUnit * 24
            placeholderText: "https://wallpapers.example.org"
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
        id: approver

        title: App.tr("Approve a device")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Approve", [approval.text.trim()], "Sync1")

        QQC2.TextField {
            id: approval

            placeholderText: "BCDF-GHJK"
        }
    }

    Kirigami.Dialog {
        id: sharer

        title: App.tr("Share the sync key with %1").arg(dialogs.device.label ?? dialogs.device.name ?? "")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("ShareKey", [dialogs.device.id, digits.text], "Sync1")

        QQC2.TextField {
            id: digits

            placeholderText: App.tr("The 12 digits shown on that device")
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
}
