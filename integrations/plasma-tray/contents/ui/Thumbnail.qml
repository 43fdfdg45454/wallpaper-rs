import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents3

Rectangle {
    id: thumbnail

    required property var daemon
    required property string item
    required property string source
    required property bool mature
    required property bool scene

    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.2)
    clip: true

    Image {
        anchors.fill: parent
        anchors.margins: parent.border.width
        source: thumbnail.source
        sourceSize: thumbnail.mature ? Qt.size(12, 12) : undefined
        asynchronous: true
        fillMode: Image.PreserveAspectCrop
    }

    PlasmaComponents3.ToolButton {
        anchors.fill: parent
        visible: thumbnail.mature
        icon.name: "view-private"
        text: i18n("Mature content: click the picture to always show this package")
        display: PlasmaComponents3.AbstractButton.IconOnly
        PlasmaComponents3.ToolTip.text: text
        PlasmaComponents3.ToolTip.visible: hovered || visualFocus
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
