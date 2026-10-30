import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: screens

    property var playlists: []

    function load() {
        App.query("List", [], "Playlists1", reply => screens && (screens.playlists = reply.playlists));
    }

    Component.onCompleted: load()

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "playlists")
                screens.load();
        }
    }

    Kirigami.Heading {
        level: 2
        text: App.tr("Screens")
    }

    Repeater {
        model: App.outputs.concat([""])

        delegate: OutputRow {
            required property string modelData

            Layout.fillWidth: true
            daemon: App
            name: modelData
            info: (App.state.outputs ?? {})[modelData] ?? {}
            playlists: screens.playlists
        }
    }
}
