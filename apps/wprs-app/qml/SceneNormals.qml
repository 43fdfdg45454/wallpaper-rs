import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    required property var page
    required property var node
    property var shown: null

    function args() {
        return {
            "layer": node.id,
            "strength": Math.round(strength.value * 10) / 10,
            "blur": Math.round(blur.value * 10) / 10
        };
    }

    function load() {
        shown = JSON.parse(page.editor.rig(node.id, JSON.stringify({
            "normals": args()
        })) || "null");
    }

    title: App.tr("Generate normals from the picture")
    standardButtons: Kirigami.Dialog.Cancel
    onOpened: load()
    customFooterActions: [
        Kirigami.Action {
            text: App.tr("Generate")
            icon.name: "dialog-ok"
            onTriggered: {
                dialog.page.run("normals", dialog.args());
                dialog.close();
            }
        }
    ]

    ColumnLayout {
        GridLayout {
            columns: 2

            QQC2.Label {
                text: App.tr("Strength")
            }

            QQC2.Slider {
                id: strength

                Layout.fillWidth: true
                from: 0
                to: 16
                value: 2
                onMoved: again.restart()
            }

            QQC2.Label {
                text: App.tr("Blur")
            }

            QQC2.Slider {
                id: blur

                Layout.fillWidth: true
                from: 0
                to: 32
                value: 1
                onMoved: again.restart()
            }
        }

        RowLayout {
            Image {
                source: dialog.shown?.relief ?? ""
                cache: false
            }

            Image {
                source: dialog.shown?.normals ?? ""
                cache: false
            }

            Image {
                source: dialog.shown?.sphere ?? ""
                cache: false
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: App.tr("The relief is lit from the top left; the map is kept as a picture of the package.")
        }

        Timer {
            id: again

            interval: 250
            onTriggered: dialog.load()
        }
    }
}
