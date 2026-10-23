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
    readonly property bool scene: info.kind === "scene"
    readonly property bool tunable: scene || (info.kind === "video" && pkg.kind === "video")
    readonly property string sound: info.sound ?? ""
    readonly property bool muted: daemon.state?.muted === true
    property string thumb
    property var details: ({})
    readonly property var pkg: details.package ?? {}
    readonly property var confirmed: pkg.confirmed ?? []
    readonly property bool favorite: details.favorite === true
    readonly property bool mature: pkg.content === "mature" && !confirmed.includes("mature")
    readonly property bool allowed: confirmed.includes("photosensitive")

    function load() {
        const item = row.item;
        if (!item)
            return;
        daemon.query("GetItem", [item], "Library1", reply => {
            if (!row || row.item !== item)
                return;
            row.details = reply.item ?? {};
            daemon.query("Thumbnail", [item, 256], "Library1", answer => {
                if (row && row.item === item)
                    row.thumb = answer[0] ?? "";
            });
        });
    }

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

    Timer {
        id: debounce

        interval: 200
        onTriggered: {
            row.daemon.act("SetVolume", [loudness.value]);
            row.daemon.act("SetMuted", [false]);
        }
    }

    Thumbnail {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 6
        Layout.preferredHeight: Kirigami.Units.gridUnit * 4
        visible: row.item !== ""
        daemon: row.daemon
        item: row.item
        source: row.daemon.url(row.thumb)
        mature: row.mature
        scene: row.scene
    }

    ColumnLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.fillWidth: true
            elide: Text.ElideMiddle
            font.bold: true
            text: row.name ? row.name + (row.info.width ? " · " + row.info.width + "×" + row.info.height : "") : row.daemon.tr("Every screen")
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.name !== ""
            elide: Text.ElideMiddle
            text: row.info.error || (row.playing ? row.daemon.tr("Playlist %1 (%2 of %3)", row.playing.name, row.playing.position + 1, row.playing.count) : (row.info.image ?? "").split("/").pop()) || row.daemon.tr("Background color")
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.pkg.photosensitive === true
            wrapMode: Text.Wrap
            text: row.allowed ? row.daemon.tr("Flashes or shows fast patterns; you allowed it") : row.daemon.tr("Flashes or shows fast patterns that can trigger seizures")
            color: row.allowed ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.negativeTextColor
            font: Kirigami.Theme.smallFont
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.mature
            wrapMode: Text.Wrap
            text: row.daemon.tr("Mature content: click the picture to always show this package")
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.credit !== null
            elide: Text.ElideRight
            text: row.credit ? [row.credit.title, row.credit.attribution, row.credit.license].filter(t => t).join(" · ") : ""
            textFormat: Text.PlainText
            font: Kirigami.Theme.smallFont
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: (row.info.shown_fit ?? row.info.fit) !== row.info.fit
            wrapMode: Text.Wrap
            text: row.daemon.tr("This picture has its own placement: %1", row.daemon.value(row.info.shown_fit ?? ""))
            textFormat: Text.PlainText
            color: Kirigami.Theme.disabledTextColor
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
                id: settings

                visible: row.tunable
                checkable: true
                checked: row.daemon.checking === true
                icon.name: "configure"
                text: row.scene ? row.daemon.tr("Scene settings") : row.daemon.tr("Video settings")
            }

            QQC2.ToolButton {
                visible: row.sound !== ""
                icon.name: row.muted ? "audio-volume-muted" : "audio-volume-high"
                text: row.muted ? row.daemon.tr("Unmute") : row.daemon.tr("Mute")
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: row.sound === "elsewhere" ? row.daemon.tr("Another screen plays the sound of this scene") : text
                QQC2.ToolTip.visible: hovered
                onClicked: row.daemon.act("SetMuted", [!row.muted])
            }

            QQC2.Slider {
                id: loudness

                visible: row.sound !== ""
                from: 0
                to: 1
                value: row.daemon.state?.volume ?? 0
                onMoved: debounce.restart()
            }

            QQC2.ToolButton {
                visible: row.item !== ""
                icon.name: row.favorite ? "emblem-favorite" : "bookmark-new"
                text: row.favorite ? row.daemon.tr("Unfavorite") : row.daemon.tr("Favorite")
                onClicked: row.daemon.act("SetFavorite", [[row.item], !row.favorite], "Library1")
            }
        }

        Loader {
            Layout.fillWidth: true
            active: row.tunable && settings.checked
            visible: active

            sourceComponent: SceneControls {
                daemon: row.daemon
                output: row.name
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
