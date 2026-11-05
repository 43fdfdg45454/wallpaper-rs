import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: media

    property var inputs: ({})
    readonly property var status: inputs.media ?? {}
    readonly property bool remote: (App.state.options ?? {})["media.remote_covers"] === true
    readonly property var using: status.using ?? []

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.tr("A scene or page shows the title, artist, album, cover and progress of what a media player on this computer plays only after you allow it for that package, and only while it is shown. Nothing of it is kept, logged or sent anywhere.")
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: !media.status.listening ? App.tr("No shown wallpaper reads the media player now.") : [App.tr("Read now for: %1", media.using.join(", ")), media.status.players ? App.tr("Media players found: %1", media.status.players) : App.tr("No media player is open."), media.status.track ? (media.status.playing ? App.tr("Something is playing.") : App.tr("Playback is paused or stopped.")) : "", media.status.cover ? App.tr("A cover is shown.") : ""].filter(t => t).join("\n")
    }

    QQC2.Switch {
        text: App.tr("Fetch covers from the internet")
        checked: media.remote
        onToggled: App.act("SetOption", ["media.remote_covers", String(checked)], "Daemon1")
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        color: Kirigami.Theme.disabledTextColor
        text: App.tr("Some players give the address of a cover on the internet instead of a file. With this on, such a cover is downloaded (at most 4 MiB, within 5 seconds) while an allowed wallpaper shows it, so that site learns that the track plays here; covers stay in memory only.")
    }
}
