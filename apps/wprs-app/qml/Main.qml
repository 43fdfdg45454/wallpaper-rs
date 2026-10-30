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
        App.query("GetInputs", [], "Inputs1", reply => root && (root.inputs = reply));
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
    globalDrawer: Navigation {
        host: root
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

    Tour {
        host: root
    }
}
