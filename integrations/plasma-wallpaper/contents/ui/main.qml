import QtQuick
import org.kde.plasma.plasmoid

WallpaperItem {
    id: root

    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    Stream {
        id: stream

        anchors.fill: parent
        covered: coverage.covered
    }

    Text {
        anchors.centerIn: parent
        width: (parent?.width ?? 0) * 0.8
        visible: stream.error !== "" && !stream.ready
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        color: "#a0a0a0"
        text: i18nd("plasma_wallpaper_io.github.wallpaper_rs", "wallpaper-rs: %1", stream.error)
    }

    Coverage {
        id: coverage
    }
}
