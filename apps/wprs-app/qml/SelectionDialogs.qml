import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: dialogs

    required property var owner
    readonly property alias importer: importer
    readonly property alias playlist: playlist
    readonly property alias tagger: tagger
    readonly property alias remover: remover
    readonly property alias renamer: renamer
    readonly property alias rater: rater

    Kirigami.Dialog {
        id: importer

        title: App.tr("Import files")
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: App.tr("Choose files…")
                icon.name: "document-open"
                onTriggered: files.open()
            }
        ]

        QQC2.Switch {
            id: copy

            text: App.tr("Copy into the library")
        }
    }

    FileDialog {
        id: files

        fileMode: FileDialog.OpenFiles
        onAccepted: {
            App.act("Import", [Array.from(selectedFiles, file => App.path(file)), copy.checked], "Library1");
            importer.close();
        }
    }

    Kirigami.Dialog {
        id: playlist

        title: App.tr("Add to playlist")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onOpened: App.query("List", [], "Playlists1", reply => dialogs.owner.playlists = reply.playlists.filter(p => p.query === null))
        onAccepted: App.act("AddItems", [chooser.currentValue, dialogs.owner.uids], "Playlists1")

        QQC2.ComboBox {
            id: chooser

            model: dialogs.owner.playlists
            textRole: "name"
            valueRole: "uid"
        }
    }

    Kirigami.Dialog {
        id: tagger

        title: App.tr("Tags")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("SetTags", [dialogs.owner.uids, dialogs.owner.list(added.text), dialogs.owner.list(removed.text)], "Library1")

        Kirigami.FormLayout {
            QQC2.TextField {
                id: added

                Kirigami.FormData.label: App.tr("Add (comma separated):")
            }

            QQC2.TextField {
                id: removed

                Kirigami.FormData.label: App.tr("Remove:")
            }
        }
    }

    Kirigami.Dialog {
        id: remover

        title: App.tr("Remove from the library")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Remove", [dialogs.owner.uids, purge.visible && purge.checked], "Library1")

        ColumnLayout {
            QQC2.Label {
                text: App.tr("Items to remove: %1").arg(dialogs.owner.uids.length)
            }

            QQC2.CheckBox {
                id: purge

                visible: dialogs.owner.items.imported
                text: App.tr("Also delete the imported copies")
            }
        }
    }

    Kirigami.Dialog {
        id: renamer

        property string from

        title: App.tr("Rename tag")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("RenameTag", [from, name.text], "Library1")

        QQC2.TextField {
            id: name

            text: renamer.from
        }
    }

    Kirigami.Dialog {
        id: rater

        title: App.tr("Rate")
        standardButtons: Kirigami.Dialog.Close

        Stars {
            onRated: stars => {
                App.act("SetRating", [dialogs.owner.uids, stars], "Library1");
                rater.close();
            }
        }
    }
}
