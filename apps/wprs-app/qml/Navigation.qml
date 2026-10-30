import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.GlobalDrawer {
    required property var host

    collapsible: true
    actions: [
        Kirigami.Action {
            text: App.tr("Library")
            icon.name: "folder-pictures"
            checked: host.current === 0
            onTriggered: host.showPage(0)
        },
        Kirigami.Action {
            text: App.tr("Scenes")
            icon.name: "draw-freehand"
            checked: host.current === 7
            onTriggered: host.showPage(7)
        },
        Kirigami.Action {
            text: App.tr("Playlists")
            icon.name: "view-media-playlist"
            checked: host.current === 1
            onTriggered: host.showPage(1)
        },
        Kirigami.Action {
            text: App.tr("Profiles and presets")
            icon.name: "video-display"
            checked: host.current === 2
            onTriggered: host.showPage(2)
        },
        Kirigami.Action {
            text: App.tr("Providers")
            icon.name: "internet-services"
            checked: host.current === 3
            onTriggered: host.showPage(3)
        },
        Kirigami.Action {
            text: App.tr("Repositories")
            icon.name: "package-x-generic"
            checked: host.current === 4
            onTriggered: host.showPage(4)
        },
        Kirigami.Action {
            text: App.tr("Settings")
            icon.name: "configure"
            checked: host.current === 5
            onTriggered: host.showPage(5)
        },
        Kirigami.Action {
            text: App.tr("Account")
            icon.name: "folder-cloud"
            checked: host.current === 6
            onTriggered: host.showPage(6)
        }
    ]
}
