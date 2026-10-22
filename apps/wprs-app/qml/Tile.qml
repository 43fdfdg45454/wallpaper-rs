import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

QQC2.ItemDelegate {
    id: tile

    required property int index
    required property string uid
    required property string title
    required property string kind
    required property string color
    required property string thumb
    required property bool favorite
    required property int rating
    required property string info
    required property bool missing
    required property string broken
    required property string revoked
    required property bool selected
    required property string tags

    signal picked(bool only)
    signal opened

    highlighted: selected
    QQC2.ToolTip.text: [title, info, tags, broken, revoked ? Labels.value(revoked) : ""].filter(t => t).join("\n")
    QQC2.ToolTip.visible: hovered
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    contentItem: ColumnLayout {
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: tile.color || Kirigami.Theme.alternateBackgroundColor

            Image {
                anchors.fill: parent
                source: tile.thumb
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                sourceSize.width: width * Screen.devicePixelRatio
                sourceSize.height: height * Screen.devicePixelRatio
            }

            Row {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: Kirigami.Units.smallSpacing

                Repeater {
                    model: [tile.favorite && "emblem-favorite", tile.kind === "video" && "media-playback-start", tile.kind === "package" && "package-x-generic", (tile.missing || tile.broken) && "emblem-error", tile.revoked && "emblem-unavailable"].filter(icon => icon)

                    delegate: Kirigami.Icon {
                        required property string modelData

                        width: Kirigami.Units.iconSizes.small
                        height: width
                        source: modelData
                    }
                }
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: tile.title
            elide: Text.ElideRight
            horizontalAlignment: Text.AlignHCenter
            color: tile.missing ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.textColor
        }
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onClicked: mouse => tile.picked(!(mouse.modifiers & (Qt.ControlModifier | Qt.ShiftModifier)))
        onDoubleClicked: tile.opened()
    }
}
