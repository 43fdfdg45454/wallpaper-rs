import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Rectangle {
    id: thumbnail

    required property var daemon
    required property string item
    required property string source
    required property bool mature
    required property bool scene
    property string animated
    property bool moving

    color: Kirigami.Theme.alternateBackgroundColor

    Image {
        anchors.fill: parent
        source: thumbnail.source
        sourceSize: thumbnail.mature ? Qt.size(12, 12) : undefined
        asynchronous: true
        fillMode: Image.PreserveAspectCrop
    }

    AnimatedImage {
        anchors.fill: parent
        anchors.margins: parent.border.width
        visible: thumbnail.animated !== "" && !thumbnail.mature && status === AnimatedImage.Ready
        source: thumbnail.animated && !thumbnail.mature ? thumbnail.animated : ""
        playing: visible && thumbnail.moving && thumbnail.Window.visibility !== Window.Hidden
        cache: false
        fillMode: Image.PreserveAspectCrop
    }

    QQC2.ToolButton {
        anchors.fill: parent
        visible: thumbnail.mature
        icon.name: "view-private"
        text: thumbnail.daemon.tr("Mature content: click the picture to always show this package")
        display: QQC2.AbstractButton.IconOnly
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: hovered || visualFocus
        onClicked: thumbnail.daemon.act("Confirm", [[thumbnail.item], "mature"], "Library1")
    }

    Kirigami.Icon {
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Kirigami.Units.smallSpacing
        width: Kirigami.Units.iconSizes.small
        height: width
        visible: thumbnail.scene
        source: "view-media-visualization"
    }
}
