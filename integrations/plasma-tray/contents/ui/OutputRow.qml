import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

RowLayout {
    id: row

    required property var daemon
    required property string name
    required property var info
    required property var playlists
    readonly property string item: info.item ?? ""
    readonly property var playing: info.playlist ?? null
    readonly property var credit: info.credit ?? null
    readonly property var fits: daemon.schema.fits ?? ["fill", "fit", "stretch", "center", "tile"]
    property string thumb
    property bool favorite

    function load() {
        const item = row.item;
        if (item) {
            daemon.query("Thumbnail", [item, 256], "Library1", reply => {
                if (row.item === item)
                    row.thumb = reply[0] ?? "";
            });
            daemon.query("GetItem", [item], "Library1", reply => {
                if (row.item === item)
                    row.favorite = reply.item?.favorite === true;
            });
        }
    }

    onItemChanged: {
        thumb = "";
        Qt.callLater(load);
    }
    Component.onCompleted: Qt.callLater(load)

    Connections {
        target: row.daemon

        function onUpdated(topic, ids) {
            if (topic === "library" && (ids.length === 0 || ids.includes(row.item)))
                row.load();
        }
    }

    Rectangle {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 6
        Layout.preferredHeight: Kirigami.Units.gridUnit * 4
        visible: row.item !== ""
        color: Kirigami.Theme.alternateBackgroundColor

        Image {
            anchors.fill: parent
            source: row.daemon.url(row.thumb)
            asynchronous: true
            fillMode: Image.PreserveAspectCrop
        }
    }

    ColumnLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.fillWidth: true
            elide: Text.ElideMiddle
            font.bold: true
            text: row.name ? row.name + (row.info.width ? " · " + row.info.width + "×" + row.info.height : "") : row.daemon.tr("Every screen")
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.name !== ""
            elide: Text.ElideMiddle
            text: row.info.error || (row.playing ? row.daemon.tr("Playlist %1 (%2 of %3)", row.playing.name, row.playing.position + 1, row.playing.count) : (row.info.image ?? "").split("/").pop()) || row.daemon.tr("Background color")
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.credit !== null
            elide: Text.ElideRight
            text: row.credit ? [row.credit.title, row.credit.attribution, row.credit.license].filter(t => t).join(" · ") : ""
            font: Kirigami.Theme.smallFont
        }

        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            QQC2.Button {
                text: row.daemon.tr("Choose…")
                icon.name: "document-open"
                onClicked: files.open()
            }

            QQC2.ComboBox {
                model: row.fits.map(fit => ({
                            "value": fit,
                            "text": row.daemon.value(fit)
                        }))
                textRole: "text"
                valueRole: "value"
                currentIndex: Math.max(0, row.fits.indexOf(row.info.fit ?? "fill"))
                onActivated: row.daemon.act("SetWallpaper", ["", row.name, currentValue])
            }

            QQC2.Button {
                text: row.daemon.tr("Background color")
                icon.name: "color-picker"
                onClicked: colors.open()
            }

            QQC2.ComboBox {
                model: [row.daemon.tr("No playlist")].concat(row.playlists.map(p => p.name))
                currentIndex: row.playing ? row.playlists.findIndex(p => p.uid === row.playing.uid) + 1 : 0
                onActivated: index => index > 0 ? row.daemon.act("Start", [row.playlists[index - 1].uid, row.name], "Playlists1") : row.daemon.act("Stop", [row.name], "Playlists1")
            }

            QQC2.ToolButton {
                visible: row.playing !== null
                icon.name: "media-skip-backward"
                text: row.daemon.tr("Previous")
                onClicked: row.daemon.act("Previous", [row.name], "Playlists1")
            }

            QQC2.ToolButton {
                visible: row.playing !== null
                icon.name: "media-skip-forward"
                text: row.daemon.tr("Next")
                onClicked: row.daemon.act("Next", [row.name], "Playlists1")
            }

            QQC2.ToolButton {
                visible: row.item !== ""
                icon.name: row.favorite ? "emblem-favorite" : "bookmark-new"
                text: row.favorite ? row.daemon.tr("Unfavorite") : row.daemon.tr("Favorite")
                onClicked: row.daemon.act("SetFavorite", [[row.item], !row.favorite], "Library1")
            }
        }
    }

    FileDialog {
        id: files

        nameFilters: [row.daemon.tr("Images and videos") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg)"]
        onAccepted: row.daemon.act("SetWallpaper", [row.daemon.path(selectedFile), row.name, ""])
    }

    ColorDialog {
        id: colors

        onAccepted: row.daemon.act("SetBackground", [selectedColor.toString(), row.name])
    }
}
