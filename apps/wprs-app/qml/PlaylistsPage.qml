import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Page {
    id: page

    property string query
    property string current
    property var playlists: []
    property string error
    readonly property var playlist: playlists.find(p => p.uid === current) ?? null

    function load() {
        App.query("List", [], "Playlists1", reply => {
            if (!page)
                return;
            page.playlists = reply.playlists;
            page.error = "";
            if (!page.playlist && page.playlists.length > 0)
                page.current = page.playlists[0].uid;
        }, error => page && (page.error = error.message));
    }

    title: App.tr("Playlists")
    Component.onCompleted: {
        load();
        if (query)
            creator.open();
    }
    actions: [
        Kirigami.Action {
            text: App.tr("New playlist…")
            icon.name: "list-add"
            onTriggered: creator.open()
        },
        Kirigami.Action {
            text: App.tr("Delete")
            icon.name: "edit-delete"
            enabled: page.playlist !== null
            onTriggered: App.act("Delete", [page.current], "Playlists1")
        }
    ]

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "playlists")
                page.load();
        }
    }

    RowLayout {
        anchors.fill: parent

        ListView {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 12
            Layout.fillHeight: true
            clip: true
            model: page.playlists

            delegate: QQC2.ItemDelegate {
                required property var modelData

                width: ListView.view.width
                text: modelData.name
                icon.name: modelData.query === null ? "view-media-playlist" : "tag-folder"
                highlighted: modelData.uid === page.current
                onClicked: page.current = modelData.uid
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width
                visible: parent.count === 0
                text: page.error || App.tr("No playlists yet")
            }
        }

        Kirigami.Separator {
            Layout.fillHeight: true
        }

        PlaylistEditor {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: page.playlist !== null
            playlist: page.playlist ?? {}
        }
    }

    Kirigami.Dialog {
        id: creator

        title: App.tr("New playlist")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onOpened: smart.text = page.query
        onAccepted: App.act("Create", [name.text, smart.text], "Playlists1", reply => page && (page.current = reply[0]))

        Kirigami.FormLayout {
            QQC2.TextField {
                id: name

                Kirigami.FormData.label: App.tr("Name:")
            }

            QQC2.TextField {
                id: smart

                Kirigami.FormData.label: App.tr("Smart query:")
                placeholderText: App.tr("Empty for a manual playlist")
            }
        }
    }
}
