import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: files

    required property var account
    property var reply: ({
            "folders": [],
            "files": []
        })

    function load() {
        App.query("ListFiles", [], "Sync1", reply => files && (files.reply = reply));
    }

    function state(file) {
        if (!file.stored)
            return App.tr("waiting to upload");
        return file.item ? App.tr("also on this computer") : App.tr("on the server");
    }

    visible: files.account?.encrypted === true && ["ready", "paused", "error"].includes(files.account?.state)
    Component.onCompleted: load()

    Connections {
        target: App

        function onUpdated(topic) {
            if (["account", "library", "jobs"].includes(topic))
                files.load();
        }
    }

    Kirigami.Heading {
        level: 3
        text: App.tr("Private files")
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: App.tr("Your own wallpapers that come from no repository, end-to-end encrypted: %1 of %2 used on the server.", App.bytes(files.account?.files_bytes ?? 0), App.bytes(files.account?.files_quota_bytes ?? 0))
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        opacity: 0.8
        text: App.tr("Item names on the server are keyed hashes: a device that once held the names key and has the same file can still tell that this account stores it, even after it is revoked.")
    }

    Repeater {
        model: files.reply.folders

        delegate: RowLayout {
            required property string modelData

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                text: App.tr("Folder %1", modelData)
            }

            QQC2.Button {
                text: App.tr("Stop sharing")
                icon.name: "edit-delete"
                onClicked: App.act("ShareFiles", [[modelData], false], "Sync1")
            }
        }
    }

    Repeater {
        model: files.reply.files

        delegate: RowLayout {
            required property var modelData

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                text: [modelData.name, App.bytes(modelData.size), files.state(modelData)].join(" · ")
            }

            QQC2.Button {
                visible: modelData.stored && !modelData.item
                text: App.tr("Download")
                icon.name: "download"
                onClicked: App.act("FetchFile", [modelData.id], "Sync1")
            }

            QQC2.Button {
                visible: modelData.stored
                text: App.tr("Remove from the server")
                icon.name: "edit-delete"
                onClicked: {
                    dropper.file = modelData;
                    dropper.open();
                }
            }
        }
    }

    QQC2.Button {
        text: App.tr("Share a folder…")
        icon.name: "folder-cloud"
        onClicked: folder.open()
    }

    FolderDialog {
        id: folder

        onAccepted: App.act("ShareFiles", [[App.path(selectedFolder)], true], "Sync1")
    }

    Kirigami.Dialog {
        id: dropper

        property var file: ({})

        title: App.tr("Remove %1 from the server", dropper.file.name ?? "")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("ShareFiles", [[dropper.file.shared ? dropper.file.item : dropper.file.id], false], "Sync1")

        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 24
            wrapMode: Text.Wrap
            text: dropper.file.item ? App.tr("The other devices can no longer download it; this computer keeps its copy.") : App.tr("No device holds this file: once it leaves the server it is lost for good.")
        }
    }
}
