import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: shared

    required property var account
    required property var dialogs
    property var providers: []
    readonly property bool encrypted: shared.account?.encrypted === true
    readonly property var offers: shared.account?.offers ?? []
    readonly property var recovery: ({
            "off": App.tr("No recovery passphrase: if every device is lost, so is the synced data."),
            "current": App.tr("The server keeps the current sync key, locked with your recovery passphrase."),
            "stale": App.tr("A device that may know the recovery passphrase was revoked, so the server keeps an older sync key: set a new, different passphrase.")
        })

    function load() {
        App.query("List", [], "Providers1", reply => shared && (shared.providers = reply.providers.filter(p => !!(App.schema.providers ?? []).find(k => k.kind === p.kind)?.secret)));
    }

    Component.onCompleted: load()

    Connections {
        target: App

        function onUpdated(topic) {
            if (topic === "providers")
                shared.load();
        }
    }

    Kirigami.Heading {
        level: 3
        visible: shared.offers.length > 0
        text: App.tr("Repositories added on your other devices")
    }

    Repeater {
        model: shared.offers

        delegate: RowLayout {
            required property var modelData

            Layout.fillWidth: true

            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: App.tr("%1 at %2, key %3", modelData.name, modelData.url, (modelData.fingerprints[0] ?? "").slice(0, 16))
            }

            QQC2.Button {
                text: App.tr("Add it here")
                icon.name: "list-add"
                onClicked: App.act("ConfirmRepo", [modelData.id, true], "Sync1")
            }

            QQC2.Button {
                text: App.tr("Not here")
                icon.name: "dialog-cancel"
                onClicked: App.act("ConfirmRepo", [modelData.id, false], "Sync1")
            }
        }
    }

    Kirigami.Heading {
        level: 3
        visible: shared.encrypted && shared.providers.length > 0
        text: App.tr("Provider secrets")
    }

    Repeater {
        model: shared.encrypted ? shared.providers : []

        delegate: QQC2.CheckBox {
            required property var modelData

            text: App.tr("Sync the secret of %1 with my devices, end-to-end encrypted", modelData.id)
            checked: (shared.account?.secrets ?? []).includes(modelData.id)
            onToggled: App.act("ShareSecret", [modelData.id, checked], "Sync1")
        }
    }

    Kirigami.Heading {
        level: 3
        visible: shared.encrypted
        text: App.tr("Recovery")
    }

    RowLayout {
        visible: shared.encrypted && shared.account?.state !== "needs_key"

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: shared.recovery[shared.account?.recovery] ?? ""
        }

        QQC2.Button {
            text: shared.account?.recovery === "off" ? App.tr("Set a passphrase…") : App.tr("Change the passphrase…")
            icon.name: "document-encrypt"
            onClicked: shared.dialogs.escrower.open()
        }

        QQC2.Button {
            visible: shared.account?.recovery !== "off"
            text: App.tr("Remove it")
            icon.name: "edit-delete"
            onClicked: unlocker.open()
        }
    }

    Kirigami.Dialog {
        id: unlocker

        title: App.tr("Remove the recovery passphrase")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("SetEscrow", [""], "Sync1")

        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 24
            wrapMode: Text.Wrap
            text: App.tr("Without it, only a device that holds the sync key can give it to a new one; if you lose every device, the synced data cannot be read any more.")
        }
    }
}
