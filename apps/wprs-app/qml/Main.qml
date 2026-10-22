import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.ApplicationWindow {
    id: root

    property int current: 0
    readonly property list<Component> pages: [
        Component {
            LibraryPage {}
        },
        Component {
            PlaylistsPage {}
        },
        Component {
            ProfilesPage {}
        },
        Component {
            ProvidersPage {}
        },
        Component {
            ReposPage {}
        },
        Component {
            SettingsPage {}
        }
    ]

    function showPage(index, properties) {
        current = index;
        pageStack.clear();
        pageStack.push(pages[index], properties ?? {});
    }

    title: "wallpaper-rs"
    width: Kirigami.Units.gridUnit * 60
    height: Kirigami.Units.gridUnit * 40
    minimumWidth: Kirigami.Units.gridUnit * 24
    minimumHeight: Kirigami.Units.gridUnit * 20
    Component.onCompleted: showPage(0)
    footer: StatusBar {}
    header: Kirigami.InlineMessage {
        type: Kirigami.MessageType.Error
        text: App.backend.banner
        visible: text !== ""
    }
    globalDrawer: Kirigami.GlobalDrawer {
        collapsible: true
        actions: [
            Kirigami.Action {
                text: App.tr("Library")
                icon.name: "folder-pictures"
                checked: root.current === 0
                onTriggered: root.showPage(0)
            },
            Kirigami.Action {
                text: App.tr("Playlists")
                icon.name: "view-media-playlist"
                checked: root.current === 1
                onTriggered: root.showPage(1)
            },
            Kirigami.Action {
                text: App.tr("Profiles and presets")
                icon.name: "video-display"
                checked: root.current === 2
                onTriggered: root.showPage(2)
            },
            Kirigami.Action {
                text: App.tr("Providers")
                icon.name: "internet-services"
                checked: root.current === 3
                onTriggered: root.showPage(3)
            },
            Kirigami.Action {
                text: App.tr("Repositories")
                icon.name: "package-x-generic"
                checked: root.current === 4
                onTriggered: root.showPage(4)
            },
            Kirigami.Action {
                text: App.tr("Settings")
                icon.name: "configure"
                checked: root.current === 5
                onTriggered: root.showPage(5)
            }
        ]
    }

    Connections {
        target: App.backend

        function onActivated() {
            if (root.visibility === Window.Minimized)
                root.showNormal();
            root.raise();
            root.requestActivate();
        }
    }

    Connections {
        target: App

        function onNavigate(page, query) {
            root.showPage(page, {
                "query": query
            });
        }
    }

    Timer {
        property int step: 0

        interval: 300
        repeat: true
        running: App.checking
        onTriggered: {
            if (step >= root.pages.length)
                return Qt.quit();
            root.showPage(step++);
            const page = root.pageStack.currentItem;
            if (page.exercise)
                page.exercise();
        }
    }
}
