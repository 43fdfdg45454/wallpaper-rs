import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: layers

    required property var page
    readonly property var relief: page.relief
    readonly property bool split: page.splitOn && relief.other !== ""

    Image {
        anchors.fill: parent
        visible: layers.page.overlayOn
        opacity: layers.page.overlayOpacity
        source: layers.relief.overlay
        fillMode: Image.Stretch
        cache: false
    }

    Rectangle {
        anchors.fill: parent
        visible: layers.page.layerPick >= 0 && layers.relief.shown !== ""
        color: Kirigami.Theme.backgroundColor

        Image {
            anchors.fill: parent
            source: layers.relief.shown
            fillMode: Image.Stretch
            cache: false
        }

        Item {
            x: parent.width * layers.page.splitAt
            width: parent.width - x
            height: parent.height
            visible: layers.split
            clip: true

            Image {
                x: -parent.x
                width: layers.width
                height: layers.height
                source: layers.relief.other
                fillMode: Image.Stretch
                cache: false
            }
        }

        Rectangle {
            x: parent.width * layers.page.splitAt - width / 2
            width: 3
            height: parent.height
            visible: layers.split
            color: Kirigami.Theme.highlightColor

            MouseArea {
                anchors.fill: parent
                anchors.margins: -Kirigami.Units.largeSpacing
                cursorShape: Qt.SplitHCursor
                onPositionChanged: mouse => {
                    const to = mapToItem(layers, mouse.x, 0).x / layers.width;
                    layers.page.splitAt = Math.max(0.02, Math.min(0.98, to));
                }
            }
        }

        QQC2.Label {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.margins: Kirigami.Units.smallSpacing
            visible: layers.split
            text: App.tr("LaMa")
            color: Kirigami.Theme.highlightedTextColor
            background: Rectangle {
                color: Kirigami.Theme.highlightColor
            }
        }

        QQC2.Label {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: Kirigami.Units.smallSpacing
            visible: layers.split
            text: App.tr("Simple fill")
            color: Kirigami.Theme.highlightedTextColor
            background: Rectangle {
                color: Kirigami.Theme.highlightColor
            }
        }
    }

    Image {
        anchors.fill: parent
        visible: layers.page.outlineOn || layers.page.cutsLive !== null
        opacity: 0.85
        source: layers.relief.outline
        fillMode: Image.Stretch
        cache: false
    }
}
