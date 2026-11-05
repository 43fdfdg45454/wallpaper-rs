import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

QQC2.ScrollView {
    id: player

    required property var page
    readonly property var media: SceneLook.media ?? SceneLook.sample
    readonly property var pictures: (page.view.assets ?? []).filter(a => a.kind === "image" && !a.media)
    readonly property var placeholder: (page.view.assets ?? []).find(a => a.media === "current")
    readonly property var colors: JSON.parse(page.editor.rig("", JSON.stringify({
        "palette": player.media.cover || (player.placeholder?.path ?? "")
    })) || "null")

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

            QQC2.ComboBox {
                Kirigami.FormData.label: App.tr("Cover")
                model: [App.tr("None")].concat(player.pictures.map(p => p.id))
                currentIndex: 1 + player.pictures.findIndex(p => p.path === player.media.cover)
                onActivated: index => player.change("cover", index > 0 ? player.pictures[index - 1].path : "")
            }

            QQC2.Switch {
                Kirigami.FormData.label: App.tr("Playing")
                checked: player.media.playing === true
                onToggled: player.change("playing", checked)
            }

            Row {
                Kirigami.FormData.label: App.tr("Cover colors")
                spacing: Kirigami.Units.smallSpacing
                visible: Array.isArray(player.colors)

                Repeater {
                    model: [App.tr("Primary"), App.tr("Secondary"), App.tr("Tertiary"), App.tr("Text"), App.tr("Contrast")]

                    delegate: Rectangle {
                        required property int index
                        required property string modelData

                        width: Kirigami.Units.gridUnit * 1.5
                        height: width
                        radius: Kirigami.Units.smallSpacing
                        border.color: Kirigami.Theme.textColor
                        color: (player.colors ?? [])[index] ?? "transparent"
                        QQC2.ToolTip.text: modelData + " " + (player.colors ?? [])[index]
                        QQC2.ToolTip.visible: hover.hovered

                        HoverHandler {
                            id: hover
                        }
                    }
                }
            }
        }
    }
}
