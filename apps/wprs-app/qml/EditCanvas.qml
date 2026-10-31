import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: canvas

    required property var page
    property int section
    readonly property var editor: page.editor
    readonly property bool whole: section === 0
    readonly property var crop: page.edit.geometry?.crop ?? [0, 0, 1, 1]
    readonly property var outputs: page.edit.outputs ?? {}
    readonly property var troubles: ({
            "gpu": App.tr("This computer has no graphics card the app can use. Show on the desktop still shows the edit."),
            "decode": App.tr("This file is not a picture the app can read."),
            "draw": App.tr("The graphics card could not draw this edit.")
        })

    spacing: 0
    onWholeChanged: editor.view(whole)
    Component.onCompleted: editor.view(whole)

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        color: Kirigami.Theme.alternateBackgroundColor
        clip: true

        Image {
            id: shown

            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            source: canvas.editor.picture
            fillMode: Image.PreserveAspectFit
            cache: false
        }

        Item {
            x: shown.x + (shown.width - shown.paintedWidth) / 2
            y: shown.y + (shown.height - shown.paintedHeight) / 2
            width: shown.paintedWidth
            height: shown.paintedHeight
            visible: shown.status === Image.Ready && canvas.editor.whole === canvas.whole
            clip: true

            EditFrame {
                visible: canvas.whole
                dim: true
                rect: canvas.crop
                onMoved: rect => canvas.page.run("crop", {
                        "rect": rect,
                        "aspect": canvas.page.aspect || undefined
                    })
            }

            EditMaskHandles {
                anchors.fill: parent
                visible: canvas.section === 5 && !canvas.editor.comparing
                page: canvas.page
                matrix: canvas.page.values.view?.[canvas.editor.whole ? "whole" : "crop"] ?? [[1, 0, 0], [0, 1, 0], [0, 0, 1]]
            }

            Repeater {
                model: canvas.section === 1 ? Object.keys(canvas.outputs) : []

                delegate: EditFrame {
                    required property string modelData
                    readonly property var screen: App.state.outputs?.[modelData]

                    rect: canvas.outputs[modelData].crop
                    label: modelData
                    onMoved: rect => canvas.page.run("crop", {
                            "output": modelData,
                            "rect": rect,
                            "aspect": screen ? screen.width + ":" + screen.height : undefined
                        })
                }
            }
        }

        QQC2.BusyIndicator {
            anchors.centerIn: parent
            running: canvas.editor.ready && canvas.editor.picture === "" && canvas.editor.trouble === "" && canvas.editor.error === ""
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: canvas.editor.picture === "" && canvas.editor.trouble !== ""
            icon.name: "image-x-generic"
            text: App.tr("The picture cannot be drawn here")
            explanation: canvas.troubles[canvas.editor.trouble] ?? ""
        }
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: canvas.editor.picture !== "" && canvas.editor.trouble !== ""
        type: Kirigami.MessageType.Warning
        showCloseButton: false
        text: App.tr("The picture shown is not up to date. %1", canvas.troubles[canvas.editor.trouble] ?? "")
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: canvas.editor.changed
        type: Kirigami.MessageType.Warning
        showCloseButton: false
        text: App.tr("This picture changed after it was edited, so the edit may no longer fit it.")
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: canvas.editor.live
        showCloseButton: false
        text: App.tr("The desktop shows this edit too, wherever this picture is the wallpaper.")
    }
}
