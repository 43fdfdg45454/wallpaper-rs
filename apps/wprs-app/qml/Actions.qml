import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: tools

    required property ItemModel items
    property var uids: []
    property var playlists: []
    readonly property list<QtObject> actions: [
        Kirigami.Action {
            text: App.tr("Import…")
            icon.name: "document-import"
            onTriggered: dialogs.importer.open()
        },
        Kirigami.Action {
            text: App.tr("Rescan all")
            icon.name: "view-refresh"
            onTriggered: App.act("Rescan", [""], "Library1")
        },
        Kirigami.Action {
            text: App.tr("Details")
            icon.name: "documentinfo"
            enabled: tools.items.selected === 1
            onTriggered: tools.inspect(tools.take()[0])
        },
        Kirigami.Action {
            text: App.tr("Apply to…")
            icon.name: "preferences-desktop-wallpaper"
            enabled: tools.items.selected === 1
            onTriggered: apply.show(tools.take()[0])
        },
        Kirigami.Action {
            text: App.tr("Add to playlist…")
            icon.name: "view-media-playlist"
            enabled: tools.items.selected > 0
            onTriggered: {
                tools.take();
                dialogs.playlist.open();
            }
        },
        Kirigami.Action {
            text: App.tr("Tags…")
            icon.name: "tag"
            enabled: tools.items.selected > 0
            onTriggered: {
                tools.take();
                dialogs.tagger.open();
            }
        },
        Kirigami.Action {
            text: tools.items.favorite ? App.tr("Unfavorite") : App.tr("Favorite")
            icon.name: "emblem-favorite"
            enabled: tools.items.selected > 0
            onTriggered: App.act("SetFavorite", [tools.take(), !tools.items.favorite], "Library1")
        },
        Kirigami.Action {
            text: App.tr("Rate…")
            icon.name: "rating"
            enabled: tools.items.selected > 0
            onTriggered: {
                tools.take();
                dialogs.rater.open();
            }
        },
        Kirigami.Action {
            text: App.tr("Remove…")
            icon.name: "edit-delete"
            enabled: tools.items.selected > 0 && tools.items.removable
            onTriggered: {
                tools.take();
                dialogs.remover.open();
            }
        }
    ]

    function take() {
        uids = JSON.parse(items.selection());
        return uids;
    }

    function inspect(uid) {
        if (uid)
            drawer.show(uid);
    }

    function renameTag(name) {
        dialogs.renamer.from = name;
        dialogs.renamer.open();
    }

    function list(text) {
        return text.split(",").map(tag => tag.trim()).filter(tag => tag);
    }

    ItemDrawer {
        id: drawer

        parent: tools.QQC2.Overlay.overlay
        removable: tools.items.selected === 1 && tools.items.removable
        onApply: uid => apply.show(uid)
        onRemove: uid => {
            tools.uids = [uid];
            dialogs.remover.open();
        }
    }

    ApplyDialog {
        id: apply
    }

    SelectionDialogs {
        id: dialogs

        owner: tools
    }
}
