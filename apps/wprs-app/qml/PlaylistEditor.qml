import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: editor

    required property var playlist
    property int matches: -1
    readonly property string uid: playlist.uid ?? ""
    readonly property var choices: [
        {
            "value": "",
            "text": App.tr("All screens")
        }
    ].concat(App.outputs.map(name => ({
                "value": name,
                "text": name
            })))

    Timer {
        id: debounce

        interval: 250
        onTriggered: App.query("Search", [query.text, 0, 0], "Library1", reply => editor.matches = reply.total, () => editor.matches = -1)
    }

    Kirigami.FormLayout {
        Layout.fillWidth: true

        QQC2.TextField {
            Kirigami.FormData.label: App.tr("Name:")
            text: editor.playlist.name ?? ""
            onAccepted: App.act("Rename", [editor.uid, text], "Playlists1")
        }

        RowLayout {
            Kirigami.FormData.label: App.tr("Smart query:")

            QQC2.TextField {
                id: query

                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                text: editor.playlist.query ?? ""
                placeholderText: App.tr("Manual playlist")
                onTextEdited: debounce.restart()
                onAccepted: App.act("SetQuery", [editor.uid, text], "Playlists1")
            }

            QQC2.Label {
                visible: editor.matches >= 0 && query.text !== ""
                text: App.tr("Matches: %1").arg(editor.matches)
            }

            QQC2.Button {
                visible: editor.playlist.query !== null && editor.playlist.query !== undefined
                text: App.tr("Make manual")
                onClicked: App.act("SetQuery", [editor.uid, ""], "Playlists1")
            }
        }

        FieldForm {
            model: App.schema.rules ?? []
            values: editor.playlist.rules ?? {}
            onEdited: (key, value) => App.act("SetRule", [editor.uid, key, value], "Playlists1")
        }

        RowLayout {
            Kirigami.FormData.label: App.tr("Show on:")

            QQC2.ComboBox {
                id: output

                model: editor.choices
                textRole: "text"
                valueRole: "value"
            }

            QQC2.Button {
                text: App.tr("Start")
                icon.name: "media-playback-start"
                onClicked: App.act("Start", [editor.uid, output.currentValue], "Playlists1")
            }
        }

        Repeater {
            model: editor.playlist.outputs ?? []

            delegate: QQC2.Button {
                required property string modelData

                Kirigami.FormData.label: App.tr("Playing on %1:").arg(modelData)
                text: App.tr("Stop")
                icon.name: "media-playback-stop"
                onClicked: App.act("Stop", [modelData], "Playlists1")
            }
        }
    }

    PlaylistItems {
        Layout.fillWidth: true
        Layout.fillHeight: true
        uid: editor.uid
        manual: editor.playlist.query === null
        choices: editor.choices.slice(1)
    }
}
