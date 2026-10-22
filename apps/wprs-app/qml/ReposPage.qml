import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Page {
    id: page

    property var repos: []
    property string error
    property string removing

    function load() {
        App.query("List", [], "Repos1", reply => {
            page.repos = reply.repos;
            page.error = "";
        }, error => page.error = error.message);
    }

    function exercise() {
        tabs.currentIndex = 1;
    }

    title: App.tr("Repositories")
    Component.onCompleted: load()
    actions: [
        Kirigami.Action {
            text: App.tr("Add repository…")
            icon.name: "list-add"
            onTriggered: adder.open()
        },
        Kirigami.Action {
            text: App.tr("Upload package…")
            icon.name: "document-export"
            onTriggered: packages.open()
        },
        Kirigami.Action {
            text: App.tr("Refresh all")
            icon.name: "view-refresh"
            enabled: page.repos.length > 0
            onTriggered: App.act("Refresh", [""], "Repos1")
        }
    ]
    header: QQC2.TabBar {
        id: tabs

        QQC2.TabButton {
            text: App.tr("Repositories")
        }

        QQC2.TabButton {
            text: App.tr("Browse")
        }

        QQC2.TabButton {
            text: App.tr("Installed")
        }
    }

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "repos")
                page.load();
        }
    }

    StackLayout {
        anchors.fill: parent
        currentIndex: tabs.currentIndex

        RepoList {
            repos: page.repos
            error: page.error
            onRemove: repo => {
                page.removing = repo;
                remover.open();
            }
        }

        Browse {
            installer: installer
        }

        Installed {
            installer: installer
        }
    }

    Installer {
        id: installer
    }

    FileDialog {
        id: packages

        nameFilters: [App.tr("Packages (*.wpak)")]
        onAccepted: App.act("Upload", [App.path(selectedFile)], "Sync1")
    }

    Kirigami.Dialog {
        id: adder

        title: App.tr("Add repository")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Add", [url.text.trim()], "Repos1")

        QQC2.TextField {
            id: url

            implicitWidth: Kirigami.Units.gridUnit * 24
            placeholderText: "https://example.org/repo/?fingerprint=…"
        }
    }

    Kirigami.Dialog {
        id: remover

        title: App.tr("Remove repository")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Remove", [page.removing, uninstall.checked], "Repos1")

        QQC2.CheckBox {
            id: uninstall

            text: App.tr("Also uninstall its packages")
        }
    }
}
