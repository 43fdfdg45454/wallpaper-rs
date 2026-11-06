import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

RowLayout {
    id: events

    required property var line
    readonly property var list: line.shown?.events ?? []

    visible: list.length > 0
    height: Kirigami.Units.gridUnit * 1.4

    QQC2.Label {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 7
        text: App.tr("Events")
        font: Kirigami.Theme.smallFont
    }

    Item {
        id: lane

        Layout.fillWidth: true
        Layout.fillHeight: true

        SceneRepeater {
            id: listed

            rows: events.list

            delegate: Rectangle {
                required property int index
                readonly property var modelData: listed.rows[index]

                x: lane.width * Math.min(1, modelData.t / events.line.length) - width / 2
                y: 2
                width: Kirigami.Units.smallSpacing * 2
                height: lane.height - 4
                radius: width / 2
                color: Kirigami.Theme.neutralTextColor

                QQC2.Label {
                    anchors.left: parent.right
                    anchors.leftMargin: 2
                    text: parent.modelData.name
                    font: Kirigami.Theme.smallFont
                }

                QQC2.ToolTip.text: App.tr("%1 at %2 s", modelData.name, modelData.t)
                QQC2.ToolTip.visible: hover.hovered

                HoverHandler {
                    id: hover
                }

                TapHandler {
                    onTapped: events.line.seek(parent.modelData.t, false)
                }
            }
        }
    }
}
