import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    required property var host

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        QQC2.Button {
            text: host.daemon.tr("Choose…")
            icon.name: "document-open"
            onClicked: files.open()
        }

        QQC2.ComboBox {
            model: host.fits.map(fit => ({
                        "value": fit,
                        "text": host.daemon.value(fit)
                    }))
            textRole: "text"
            valueRole: "value"
            currentIndex: Math.max(0, host.fits.indexOf(host.info.fit ?? "fill"))
            onActivated: host.daemon.act("SetWallpaper", ["", host.name, currentValue])
        }

        QQC2.Button {
            text: host.daemon.tr("Background color")
            icon.name: "color-picker"
            onClicked: colors.open()
        }

        QQC2.ComboBox {
            model: [host.daemon.tr("No playlist")].concat(host.playlists.map(p => p.name))
            currentIndex: host.playing ? host.playlists.findIndex(p => p.uid === host.playing.uid) + 1 : 0
            onActivated: index => index > 0 ? host.daemon.act("Start", [host.playlists[index - 1].uid, host.name], "Playlists1") : host.daemon.act("Stop", [host.name], "Playlists1")
        }

        QQC2.ToolButton {
            visible: host.playing !== null
            icon.name: "media-skip-backward"
            text: host.daemon.tr("Previous")
            onClicked: host.daemon.act("Previous", [host.name], "Playlists1")
        }

        QQC2.ToolButton {
            visible: host.playing !== null
            icon.name: "media-skip-forward"
            text: host.daemon.tr("Next")
            onClicked: host.daemon.act("Next", [host.name], "Playlists1")
        }

        QQC2.ToolButton {
            id: settings

            visible: host.tunable
            checkable: true
            checked: host.daemon.checking === true
            icon.name: "configure"
            text: host.scene ? host.daemon.tr("Scene settings") : host.daemon.tr("Video settings")
        }

        QQC2.ToolButton {
            visible: host.sound !== ""
            icon.name: host.muted ? "audio-volume-muted" : "audio-volume-high"
            text: host.muted ? host.daemon.tr("Unmute") : host.daemon.tr("Mute")
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: host.sound === "elsewhere" ? host.daemon.tr("Another screen plays the sound of this scene") : text
            QQC2.ToolTip.visible: hovered
            onClicked: host.daemon.act("SetMuted", [!host.muted])
        }

        QQC2.Slider {
            id: loudness

            visible: host.sound !== ""
            from: 0
            to: 1
            value: host.daemon.state?.volume ?? 0
            onMoved: debounce.restart()
        }

        QQC2.ToolButton {
            visible: host.item !== ""
            icon.name: host.favorite ? "emblem-favorite" : "bookmark-new"
            text: host.favorite ? host.daemon.tr("Unfavorite") : host.daemon.tr("Favorite")
            onClicked: host.daemon.act("SetFavorite", [[host.item], !host.favorite], "Library1")
        }
    }

    Loader {
        Layout.fillWidth: true
        active: host.tunable && settings.checked
        visible: active

        sourceComponent: SceneControls {
            daemon: host.daemon
            output: host.name
        }
    }

    Timer {
        id: debounce

        interval: 200
        onTriggered: {
            host.daemon.act("SetVolume", [loudness.value]);
            host.daemon.act("SetMuted", [false]);
        }
    }

    FileDialog {
        id: files

        nameFilters: [host.daemon.tr("Images and videos") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg)"]
        onAccepted: host.daemon.act("SetWallpaper", [host.daemon.path(selectedFile), host.name, ""])
    }

    ColorDialog {
        id: colors

        onAccepted: host.daemon.act("SetBackground", [selectedColor.toString(), host.name])
    }
}
