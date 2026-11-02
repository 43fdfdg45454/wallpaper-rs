import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    required property var node
    readonly property var entry: node?.type === "adjustment" ? JSON.parse(page.editor.value(node.pointer) || "null") : null
    readonly property bool lit: entry?.rgb === true
    readonly property var canvas: page.view.canvas?.size ?? [1920, 1080]
    property var strip: null

    function edit(args) {
        page.run("rgb", Object.assign({
            "layer": node.id
        }, args));
    }

    visible: entry !== null

    Timer {
        interval: 100
        repeat: true
        running: panel.visible && panel.lit && panel.page.editor.output !== ""
        onTriggered: App.query("GetStrip", [panel.page.editor.output], "Rgb1", reply => panel && (panel.strip = reply), () => panel && (panel.strip = null))
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("RGB lighting")
    }

    QQC2.Switch {
        text: App.tr("Send what this layer captures to RGB lighting")
        checked: panel.lit
        onToggled: panel.edit({
            "on": checked
        })
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: panel.lit
        wrapMode: Text.Wrap
        text: App.tr("The capture is shrunk to at most 64 × 16 and sent to keyboards, mice and other lights through OpenRGB, at most 30 times a second, only where the user allowed it. Move the region and drag its corner on the preview to choose what is sent.")
    }

    RowLayout {
        visible: panel.lit

        QQC2.Button {
            text: App.tr("Whole canvas")
            enabled: panel.entry?.size !== undefined
            onClicked: panel.edit({
                "whole": true
            })
        }

        QQC2.Button {
            text: App.tr("A strip of it")
            enabled: panel.entry?.size === undefined
            onClicked: panel.edit({
                "size": [Math.round(panel.canvas[0]), Math.round(panel.canvas[1] / 4)]
            })
        }
    }

    QQC2.Label {
        visible: panel.lit
        text: panel.strip ? App.tr("What the lights get, %1 × %2:", panel.strip[0], panel.strip[1]) : App.tr("Play the preview to see what the lights get.")
    }

    Grid {
        visible: panel.lit && panel.strip !== null
        columns: Math.max(1, panel.strip?.[0] ?? 1)
        spacing: 1

        Repeater {
            model: panel.strip ? panel.strip[0] * panel.strip[1] : 0

            delegate: Rectangle {
                required property int index

                width: Kirigami.Units.gridUnit * 0.4
                height: width
                color: Qt.rgba(panel.strip[2][index * 3] / 255, panel.strip[2][index * 3 + 1] / 255, panel.strip[2][index * 3 + 2] / 255, 1)
            }
        }
    }
}
