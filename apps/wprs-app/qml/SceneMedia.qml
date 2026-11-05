import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

QQC2.ScrollView {
    id: player

    required property var page
    readonly property var media: SceneLook.media ?? SceneLook.sample

    function change(key, value) {
        const next = Object.assign({}, player.media);
        next[key] = value;
        SceneLook.media = next;
        SceneLook.preview(player.page);
    }

    ColumnLayout {
        width: player.availableWidth

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: App.tr("Try the scene with a made-up track instead of what a player plays. Only the preview shows it, without asking for permission.")
        }

        QQC2.Switch {
            text: App.tr("Simulated player")
            checked: SceneLook.media !== null
            onToggled: {
                SceneLook.media = checked ? Object.assign({}, SceneLook.sample) : null;
                SceneLook.preview(player.page);
            }
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true
            enabled: SceneLook.media !== null

            Repeater {
                model: [["title", App.tr("Title")], ["artist", App.tr("Artist")], ["album", App.tr("Album")]]

                delegate: QQC2.TextField {
                    required property var modelData

                    Kirigami.FormData.label: modelData[1]
                    text: player.media[modelData[0]] ?? ""
                    maximumLength: 512
                    onEditingFinished: player.change(modelData[0], text)
                }
            }
        }
    }
}
