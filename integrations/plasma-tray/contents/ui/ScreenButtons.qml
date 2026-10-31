import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents3

RowLayout {
    required property var host

    spacing: Kirigami.Units.smallSpacing

    Item {
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        visible: host.playing === null
    }

    PlasmaComponents3.Button {
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        visible: host.playing !== null
        icon.name: "media-skip-forward"
        text: i18n("Next")
        onClicked: host.daemon.act("Next", [host.name], "Playlists1")
    }

    PlasmaComponents3.Button {
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        icon.name: "document-open"
        text: i18n("Choose…")
        onClicked: files.open()
    }

    PlasmaComponents3.Button {
        Layout.fillWidth: true
        Layout.preferredWidth: 1
        enabled: host.item !== ""
        checkable: true
        checked: host.details.favorite === true
        icon.name: checked ? "emblem-favorite" : "bookmark-new"
        text: i18n("Favorite")
        PlasmaComponents3.ToolTip.text: checked ? i18n("Remove from favorites") : i18n("Add to favorites")
        PlasmaComponents3.ToolTip.visible: hovered
        onToggled: {
            host.daemon.act("SetFavorite", [[host.item], checked], "Library1");
            checked = Qt.binding(() => host.details.favorite === true);
        }
    }

    FileDialog {
        id: files

        nameFilters: [i18n("Images and videos") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg)"]
        onAccepted: host.daemon.act("SetWallpaper", [host.daemon.path(selectedFile), host.name, ""])
    }
}
