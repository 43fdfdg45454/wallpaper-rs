import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.ScrollablePage {
    id: page

    property var account: null
    property string error
    property string code
    property string address
    readonly property string phase: account?.state ?? ""
    readonly property bool keyed: ["ready", "paused", "error"].includes(phase)

    function load() {
        App.query("GetStatus", [], "Sync1", reply => {
                page.account = reply.account;
                page.error = "";
            }, error => page.error = error.message);
    }

    function link(name, plaintext) {
        App.query("Link", [name, plaintext], "Sync1", reply => {
                page.code = reply[0];
                page.address = reply[1];
            });
    }

    function exercise() {
        dialogs.linker.open();
    }

    title: App.tr("Account")
    Component.onCompleted: load()
    actions: [
        Kirigami.Action {
            text: App.tr("Add server…")
            icon.name: "list-add"
            visible: !page.account
            onTriggered: dialogs.adder.open()
        },
        Kirigami.Action {
            text: App.tr("Link this computer…")
            icon.name: "link"
            visible: page.phase === "unlinked"
            onTriggered: dialogs.linker.open()
        },
        Kirigami.Action {
            text: App.tr("Sync now")
            icon.name: "view-refresh"
            visible: page.keyed
            onTriggered: App.act("SyncNow", [], "Sync1")
        },
        Kirigami.Action {
            text: page.account?.paused ? App.tr("Resume sync") : App.tr("Pause sync")
            icon.name: page.account?.paused ? "media-playback-start" : "media-playback-pause"
            visible: page.keyed
            onTriggered: App.act("SetPaused", [!page.account.paused], "Sync1")
        },
        Kirigami.Action {
            text: App.tr("Approve a device…")
            icon.name: "dialog-ok-apply"
            visible: !!page.account?.device
            onTriggered: codes.approver.open()
        },
        Kirigami.Action {
            text: App.tr("Restore with the recovery passphrase…")
            icon.name: "document-decrypt"
            visible: page.phase === "needs_key" && page.account?.encrypted === true
            onTriggered: dialogs.restorer.open()
        },
        Kirigami.Action {
            text: App.tr("Replace the sync key")
            icon.name: "view-refresh"
            visible: page.keyed && page.account?.encrypted === true
            onTriggered: App.act("Rotate", [], "Sync1")
        },
        Kirigami.Action {
            text: App.tr("Reset synced data…")
            icon.name: "edit-clear-all"
            visible: !!page.account?.device
            onTriggered: dialogs.resetter.open()
        },
        Kirigami.Action {
            text: App.tr("Remove server…")
            icon.name: "edit-delete"
            visible: !!page.account
            onTriggered: dialogs.remover.open()
        }
    ]

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "account" || topic === "jobs")
                page.load();
        }
    }

    ColumnLayout {
        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            visible: !page.account
            icon.name: page.error ? "data-error" : "folder-cloud"
            text: page.error || App.tr("No server yet")
            explanation: page.error ? "" : App.tr("A wallpaper-rs server keeps your library, playlists and settings in sync between your computers, encrypted end to end.")
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.code !== "" && page.phase === "linking"
            type: Kirigami.MessageType.Information
            text: App.tr("Open %1 and enter the code %2, or approve it on a linked computer.").arg(page.address).arg(page.code)
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.account?.encrypted === false
            type: Kirigami.MessageType.Warning
            text: App.tr("Sync is not encrypted: the operator of the server can read your library, playlists and settings.")
        }

        AccountSummary {
            Layout.fillWidth: true
            visible: !!page.account
            account: page.account ?? {}
        }

        Repeater {
            model: page.account?.devices ?? []

            delegate: AccountDevice {
                required property var modelData

                Layout.fillWidth: true
                device: modelData
                keyed: page.keyed
                joining: page.phase === "needs_key"
                pairing: page.account?.pairing ?? null
                onShare: codes.share(modelData)
            }
        }

        AccountShared {
            Layout.fillWidth: true
            visible: !!page.account?.device
            account: page.account
            dialogs: dialogs
        }

        AccountFiles {
            Layout.fillWidth: true
            account: page.account
        }
    }

    AccountDialogs {
        id: dialogs

        page: page
    }

    AccountCodes {
        id: codes

        account: page.account
    }
}
