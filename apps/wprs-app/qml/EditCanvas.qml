import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: canvas

    required property var page
    property bool screens
    readonly property var frame: page.values.frame ?? [1, 1]
    readonly property var geometry: page.edit.geometry ?? {}
    readonly property var crop: geometry.crop ?? [0, 0, 1, 1]
    readonly property bool turned: (geometry.rotate ?? 0) % 180 === 90
    readonly property var outputs: page.edit.outputs ?? {}

    clip: true

    Rectangle {
        anchors.fill: parent
        color: Kirigami.Theme.alternateBackgroundColor
    }

    Item {
        id: box

        readonly property real ratio: canvas.frame[0] / Math.max(1, canvas.frame[1])

        anchors.centerIn: parent
        width: Math.min(canvas.width, canvas.height * ratio)
        height: width / ratio
        clip: true

        Item {
            anchors.centerIn: parent
            width: box.width * (canvas.page.values.zoom ?? 1)
            height: box.height * (canvas.page.values.zoom ?? 1)
            rotation: -(canvas.geometry.straighten ?? 0)

            Item {
                id: turn

                anchors.fill: parent

                transform: Scale {
                    origin.x: turn.width / 2
                    origin.y: turn.height / 2
                    xScale: canvas.geometry.flip?.[0] ? -1 : 1
                    yScale: canvas.geometry.flip?.[1] ? -1 : 1
                }

                Image {
                    anchors.centerIn: parent
                    width: canvas.turned ? parent.height : parent.width
                    height: canvas.turned ? parent.width : parent.height
                    rotation: canvas.geometry.rotate ?? 0
                    source: canvas.page.editor.source
                    sourceSize.width: 2048
                    fillMode: Image.Stretch
                    asynchronous: true
                }
            }
        }

        EditFrame {
            visible: !canvas.screens
            rect: canvas.crop
            onMoved: rect => canvas.page.run("crop", {
                    "rect": rect,
                    "aspect": canvas.page.aspect || undefined
                })
        }

        Item {
            x: canvas.crop[0] * box.width
            y: canvas.crop[1] * box.height
            width: canvas.crop[2] * box.width
            height: canvas.crop[3] * box.height
            visible: canvas.screens

            Repeater {
                model: Object.keys(canvas.outputs)

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
    }
}
