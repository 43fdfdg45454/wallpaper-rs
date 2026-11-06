import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: charter

    property bool fresh: true
    property string folder: ""
    property var current: null
    property var removed: []
    property alias named: name.text
    property alias roots: roots
    property alias onlines: onlines
    readonly property bool ready: !fresh || (folder !== "" && roots.keys.length > 0 && onlines.keys.length > 0)

    signal made(string folder, string charter)

    function start(state) {
        current = state;
        fresh = state === null;
        removed = [];
        roots.keys = [];
        onlines.keys = [];
        writers.keys = [];
        name.text = "";
        author.text = "";
        threshold.value = state?.policy?.threshold ?? 1;
        policy.reset();
        open();
    }

    function written() {
        const out = {
            "root_keys": roots.keys,
            "online_keys": onlines.keys,
            "removed": removed,
            "threshold": threshold.value
        };
        Object.assign(out, policy.written());
        if (fresh)
            out.name = name.text;
        if (author.text !== "" && writers.keys.length > 0)
            out.authors = {
                [author.text]: writers.keys
            };
        return JSON.stringify(out);
    }

    title: fresh ? App.tr("New static repository") : App.tr("Rotate the root")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    padding: Kirigami.Units.largeSpacing
    onAccepted: made(folder, written())
    onOpened: standardButton(Kirigami.Dialog.Ok).enabled = Qt.binding(() => charter.ready)

    Kirigami.FormLayout {
        RowLayout {
            Kirigami.FormData.label: App.tr("Folder:")
            visible: charter.fresh

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                text: charter.folder || App.tr("An empty folder")
            }

            QQC2.Button {
                icon.name: "document-open-folder"
                text: App.tr("Choose…")
                onClicked: folders.open()
            }
        }

        QQC2.TextField {
            id: name

            Kirigami.FormData.label: App.tr("Name:")
            visible: charter.fresh
        }

        RepoKeyList {
            id: roots

            Kirigami.FormData.label: App.tr("Root keys:")
            label: charter.fresh ? App.tr("At least one") : App.tr("%1 kept", charter.current?.root_keys?.length ?? 0)
        }

        QQC2.SpinBox {
            id: threshold

            Kirigami.FormData.label: App.tr("Root keys that must sign:")
            from: 1
            to: Math.max(1, roots.keys.length || (charter.current?.root_keys?.length ?? 1))
        }

        RepoKeyList {
            id: onlines

            Kirigami.FormData.label: App.tr("Online keys:")
            label: charter.fresh ? App.tr("At least one; they sign the index") : App.tr("%1 kept", charter.current?.online_keys?.length ?? 0)
        }

        Repeater {
            model: charter.fresh ? [] : Object.keys(charter.current?.authors ?? {})

            delegate: QQC2.CheckBox {
                required property string modelData
                required property int index

                Kirigami.FormData.label: index === 0 ? App.tr("Remove authors:") : ""
                text: App.tr("%1, keys %2", modelData, (charter.current?.authors?.[modelData] ?? []).join(", "))
                checked: charter.removed.includes(modelData)
                onToggled: charter.removed = checked ? charter.removed.concat([modelData]) : charter.removed.filter(id => id !== modelData)
            }
        }

        QQC2.TextField {
            id: author

            Kirigami.FormData.label: charter.fresh ? App.tr("Add an author:") : App.tr("Add an author or replace its keys:")
            placeholderText: App.tr("Author id")
        }

        RepoKeyList {
            id: writers

            label: App.tr("Public keys of that author")
        }

        RepoPolicy {
            id: policy

            Kirigami.FormData.label: App.tr("Policy:")
            current: charter.current
        }

        QQC2.Label {
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
            wrapMode: Text.Wrap
            text: charter.fresh ? App.tr("Sign the root with a root key next.") : App.tr("Sign the new root with keys of the old and of the new root, then publish.")
        }
    }

    FolderDialog {
        id: folders

        onAccepted: charter.folder = App.path(selectedFolder)
    }
}
