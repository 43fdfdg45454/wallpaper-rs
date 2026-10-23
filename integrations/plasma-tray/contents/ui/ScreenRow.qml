import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents3

ColumnLayout {
    id: row

    required property var daemon
    required property int index
    required property string name
    required property var info
    readonly property string item: info.item ?? ""
    readonly property var playing: info.playlist ?? null
    readonly property string title: details.title || info.credit?.title || String(info.image ?? "").split("/").pop() || (item ? "" : i18n("Background color"))
    property string thumb
    property var details: ({})

    function load() {
        const item = row.item;
        if (!item)
            return;
        daemon.query("Thumbnail", [item, 256], "Library1", reply => {
            if (row.item === item)
                row.thumb = reply[0] ?? "";
        });
        daemon.query("GetItem", [item], "Library1", reply => {
            if (row.item === item)
                row.details = reply.item ?? {};
        });
    }

    spacing: Kirigami.Units.smallSpacing
    onItemChanged: {
        thumb = "";
        details = {};
        Qt.callLater(load);
    }
    Component.onCompleted: Qt.callLater(load)

    Connections {
        target: row.daemon

        function onUpdated(topic, ids) {
            if (topic !== "library")
                return;
            if (ids.includes(row.item))
                row.load();
            else if (ids.length === 0)
                settle.restart();
        }
    }

    Timer {
        id: settle

        interval: 1000
        onTriggered: row.load()
    }

    Kirigami.Separator {
        Layout.fillWidth: true
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        visible: row.index > 0
    }

    RowLayout {
        spacing: Kirigami.Units.largeSpacing

        Rectangle {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            Layout.preferredHeight: Kirigami.Units.gridUnit * 2.5
            color: row.info.background || Kirigami.Theme.alternateBackgroundColor
            border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.2)
            clip: true

            Image {
                anchors.fill: parent
                anchors.margins: parent.border.width
                source: row.daemon.url(row.thumb)
                asynchronous: true
                fillMode: Image.PreserveAspectCrop
            }

            Kirigami.Icon {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: Kirigami.Units.smallSpacing
                width: Kirigami.Units.iconSizes.small
                height: width
                visible: row.info.kind === "scene"
                source: "view-media-visualization"
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                text: row.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
            }

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                text: row.info.error || [row.name].concat(row.playing ? [i18n("%1 of %2", row.playing.position + 1, row.playing.count), row.playing.name] : []).join(" · ")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font: Kirigami.Theme.smallFont
                color: row.info.error ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.disabledTextColor
            }
        }
    }

    RowLayout {
        spacing: Kirigami.Units.smallSpacing

        Item {
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            visible: row.playing === null
        }

        PlasmaComponents3.Button {
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            visible: row.playing !== null
            icon.name: "media-skip-forward"
            text: i18n("Next")
            onClicked: row.daemon.act("Next", [row.name], "Playlists1")
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
            enabled: row.item !== ""
            checkable: true
            checked: row.details.favorite === true
            icon.name: checked ? "emblem-favorite" : "bookmark-new"
            text: i18n("Favorite")
            PlasmaComponents3.ToolTip.text: checked ? i18n("Remove from favorites") : i18n("Add to favorites")
            PlasmaComponents3.ToolTip.visible: hovered
            onToggled: {
                row.daemon.act("SetFavorite", [[row.item], checked], "Library1");
                checked = Qt.binding(() => row.details.favorite === true);
            }
        }
    }

    FileDialog {
        id: files

        nameFilters: [i18n("Images and videos") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg)"]
        onAccepted: row.daemon.act("SetWallpaper", [row.daemon.path(selectedFile), row.name, ""])
    }
}
