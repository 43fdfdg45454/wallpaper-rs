import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.ApplicationWindow {
    id: root

    property int current: 0
    property var inputs: ({})
    property var asked: ({})
    readonly property var names: ["library", "playlists", "profiles", "providers", "repos", "settings", "account", "scenes"]
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
        },
        Component {
            AccountPage {}
        },
        Component {
            ScenesPage {}
        }
    ]

    function open(component, properties) {
        const old = pageStack.currentItem;
        pageStack.clear();
        old?.destroy();
        pageStack.push(component.createObject(pageStack, properties ?? {}));
    }

    function loadInputs() {
        App.query("GetInputs", [], "Inputs1", reply => root.inputs = reply);
    }

    function showPage(index, properties) {
        current = index;
        open(pages[index], properties);
    }

    function activate(page) {
        const index = names.indexOf(page);
        if (index >= 0 && index !== current && pageStack.currentItem?.editor?.dirty !== true)
            showPage(index);
        if (visibility === Window.Minimized)
            showNormal();
        raise();
        requestActivate();
    }

    title: "wallpaper-rs"
    width: Kirigami.Units.gridUnit * 60
    height: Kirigami.Units.gridUnit * 40
    minimumWidth: Kirigami.Units.gridUnit * 24
    minimumHeight: Kirigami.Units.gridUnit * 20
    Component.onCompleted: {
        showPage(Math.max(0, names.indexOf(App.backend.requested())));
        loadInputs();
    }
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
                text: App.tr("Scenes")
                icon.name: "draw-freehand"
                checked: root.current === 7
                onTriggered: root.showPage(7)
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
            },
            Kirigami.Action {
                text: App.tr("Account")
                icon.name: "folder-cloud"
                checked: root.current === 6
                onTriggered: root.showPage(6)
            }
        ]
    }

    Connections {
        target: App.backend

        function onActivated(page) {
            root.activate(page);
        }

        function onChanged(topic) {
            if (topic === "inputs" || topic === "state")
                root.loadInputs();
        }
    }

    Consent {
        id: consent

        visible: false
        daemon: App
        inputs: root.inputs
        onAskChanged: {
            if (ask.scene === "")
                return asking.close();
            if (root.asked[ask.scene])
                return;
            root.asked[ask.scene] = true;
            asking.open();
        }
    }

    Kirigami.PromptDialog {
        id: asking

        title: App.tr("Allow this scene?")
        subtitle: consent.text
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: App.tr("Allow")
                icon.name: "dialog-ok-apply"
                onTriggered: {
                    consent.answer(true);
                    asking.close();
                }
            },
            Kirigami.Action {
                text: App.tr("Not now")
                icon.name: "dialog-cancel"
                onTriggered: {
                    consent.answer(false);
                    asking.close();
                }
            }
        ]
    }

    Connections {
        target: App

        function onNavigate(page, query) {
            root.showPage(page, {
                "query": query
            });
        }

        function onEdit(item) {
            root.current = -1;
            root.open(editor, {
                "item": item
            });
        }

        function onCompose(folder, starter, text) {
            root.current = -1;
            root.open(composer, {
                "folder": folder,
                "starter": starter,
                "heading": starter ? text : "",
                "source": starter ? "" : text
            });
        }
    }

    Component {
        id: editor

        EditPage {}
    }

    Component {
        id: composer

        ScenePage {}
    }

    Timer {
        property int step: 0

        function keeps(page) {
            const shown = root.pageStack.currentItem;
            root.activate(page);
            if (root.pageStack.currentItem !== shown)
                throw new Error("a request for the " + page + " page replaced the page shown");
        }

        interval: 300
        repeat: true
        running: App.checking
        onTriggered: {
            const at = step++;
            if (at > root.pages.length + 1) {
                Qt.quit();
                if (root.pageStack.currentItem.editor?.dirty !== true)
                    throw new Error("the editor has no unsaved edits to keep");
                return keeps("settings");
            }
            if (at === root.pages.length)
                return App.query("GetItem", [""], "Library1", reply => App.edit(reply.item));
            if (at < root.pages.length) {
                root.showPage(at);
                keeps(root.names[at]);
            }
            const page = root.pageStack.currentItem;
            if (page.exercise)
                page.exercise();
        }
    }
}
