import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.AbstractCard {
    id: card

    required property string name
    required property var playlists
    readonly property var info: (App.state.outputs ?? {})[name] ?? {}
    readonly property string item: info.item ?? ""
    readonly property var playing: info.playlist ?? null
    readonly property var fits: App.schema.fits ?? ["fill", "fit", "stretch", "center", "tile"]
    property string thumb

    function load() {
        const item = card.item;
        thumb = "";
        if (item)
            App.query("Thumbnail", [item, 256], "Library1", reply => {
                if (card.item === item)
                    card.thumb = reply[0];
            });
    }

    onItemChanged: load()
    Component.onCompleted: load()

    contentItem: RowLayout {
        Rectangle {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
            Layout.preferredHeight: Kirigami.Units.gridUnit * 4
            visible: card.name !== ""
            color: Kirigami.Theme.alternateBackgroundColor

            Image {
                anchors.fill: parent
                source: App.url(card.thumb)
                asynchronous: true
                fillMode: Image.PreserveAspectCrop
            }
        }

        ColumnLayout {
            Layout.fillWidth: true

            Kirigami.Heading {
                level: 4
                text: card.name ? card.name + " · " + card.info.width + "×" + card.info.height : App.tr("Every screen")
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: card.name !== ""
                elide: Text.ElideMiddle
                text: card.info.error || (card.playing ? App.tr("Playlist %1 (%2 of %3)").arg(card.playing.name).arg(card.playing.position + 1).arg(card.playing.count) : (card.info.image ?? "").split("/").pop()) || App.tr("Background color")
            }

            QQC2.Label {
                visible: !!card.info.credit
                text: card.info.credit ? [card.info.credit.title, card.info.credit.attribution, card.info.credit.license].filter(t => t).join(" · ") : ""
                font: Kirigami.Theme.smallFont
            }

            Flow {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                QQC2.Button {
                    text: App.tr("Choose…")
                    icon.name: "document-open"
                    onClicked: files.open()
                }

                QQC2.ComboBox {
                    model: card.fits
                    displayText: Labels.value(currentValue)
                    currentIndex: Math.max(0, card.fits.indexOf(card.info.fit ?? "fill"))
                    onActivated: App.act("SetWallpaper", ["", card.name, currentValue])
                }

                QQC2.Button {
                    text: App.tr("Background color")
                    icon.name: "color-picker"
                    onClicked: colors.open()
                }

                QQC2.ComboBox {
                    model: [App.tr("No playlist")].concat(card.playlists.map(p => p.name))
                    currentIndex: card.playing ? card.playlists.findIndex(p => p.uid === card.playing.uid) + 1 : 0
                    onActivated: index => index > 0 ? App.act("Start", [card.playlists[index - 1].uid, card.name], "Playlists1") : App.act("Stop", [card.name], "Playlists1")
                }

                QQC2.ToolButton {
                    visible: card.playing !== null
                    icon.name: "media-skip-backward"
                    text: App.tr("Previous")
                    onClicked: App.act("Previous", [card.name], "Playlists1")
                }

                QQC2.ToolButton {
                    visible: card.playing !== null
                    icon.name: "media-skip-forward"
                    text: App.tr("Next")
                    onClicked: App.act("Next", [card.name], "Playlists1")
                }
            }
        }
    }

    FileDialog {
        id: files

        nameFilters: [App.tr("Images and videos") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg)"]
        onAccepted: App.act("SetWallpaper", [App.path(selectedFile), card.name, ""])
    }

    ColorDialog {
        id: colors

        onAccepted: App.act("SetBackground", [selectedColor.toString(), card.name])
    }
}
