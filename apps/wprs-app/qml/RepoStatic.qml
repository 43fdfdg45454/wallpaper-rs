import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: stall

    property string folder: ""
    property var shown: null
    property string message: ""
    property bool failed: false
    property var pending: null

    function take(answer) {
        const reply = JSON.parse(answer);
        failed = reply.error !== undefined;
        if (failed) {
            message = reply.error;
            return;
        }
        shown = reply;
        message = reply.added ? App.tr("Added %1. Publish the repository next.", reply.added.join(", ")) : "";
    }

    function ask(text, run) {
        pending = run;
        confirm.subtitle = text;
        confirm.open();
    }

    function list(id, version, on) {
        return backend.list(folder, id, version, on);
    }

    function feature(id, note, on) {
        return backend.feature(folder, id, note, on);
    }

    function done(answer) {
        take(answer);
        if (!failed)
            message = App.tr("Changed. Publish the repository next: wprs-ctl repo publish %1 --key <online key>", folder);
    }

    Stall {
        id: backend
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.Button {
            icon.name: "document-open-folder"
            text: App.tr("Open a static repository…")
            onClicked: folders.open()
        }

        QQC2.Label {
            Layout.fillWidth: true
            elide: Text.ElideMiddle
            text: stall.shown ? stall.shown.name + " · " + stall.shown.repo_id : App.tr("A folder made with wprs-ctl repo init")
        }

        QQC2.CheckBox {
            id: unlisted

            text: App.tr("Add unlisted")
            QQC2.ToolTip.text: App.tr("Out of browsing and search; installing by id still works")
            QQC2.ToolTip.visible: hovered
        }

        QQC2.Button {
            icon.name: "list-add"
            text: App.tr("Add packages…")
            enabled: stall.shown !== null
            onClicked: wpaks.open()
        }
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: stall.message !== ""
        type: stall.failed ? Kirigami.MessageType.Error : Kirigami.MessageType.Positive
        text: stall.message
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: stall.shown?.packages ?? []
            spacing: Kirigami.Units.smallSpacing
            clip: true

            delegate: RepoStock {
                width: ListView.view.width
                stall: stall
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirm

        title: App.tr("Change the repository")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: stall.done(stall.pending())
    }

    FolderDialog {
        id: folders

        onAccepted: {
            stall.folder = App.path(selectedFolder);
            stall.take(backend.open(stall.folder));
        }
    }

    FileDialog {
        id: wpaks

        fileMode: FileDialog.OpenFiles
        nameFilters: [App.tr("Packages (*.wpak)")]
        onAccepted: stall.take(backend.add(stall.folder, JSON.stringify(selectedFiles.map(file => App.path(file))), unlisted.checked))
    }
}
