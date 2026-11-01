import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: points

    required property var tools
    readonly property var list: tools.info?.points ?? []

    RowLayout {
        Kirigami.Heading {
            Layout.fillWidth: true
            level: 4
            text: App.tr("Control points")
        }

        QQC2.ToolButton {
            icon.name: "list-add"
            text: App.tr("Add a control point")
            display: QQC2.AbstractButton.IconOnly
            enabled: points.list.length < 8
            onClicked: points.tools.edit("point", {
                "point": points.list.length,
                "at": [0, 0]
            })
        }
    }

    Repeater {
        model: points.list

        delegate: RowLayout {
            id: spot

            required property var modelData
            required property int index
            readonly property int number: Number(modelData.name.slice(1))

            QQC2.Label {
                text: spot.modelData.name
            }

            QQC2.CheckBox {
                text: App.tr("Follows the pointer")
                checked: spot.modelData.follow
                onToggled: points.tools.edit("point", {
                    "point": spot.number,
                    "follow": checked
                })
            }

            QQC2.Dial {
                implicitWidth: Kirigami.Units.gridUnit * 2
                implicitHeight: implicitWidth
                from: -180
                to: 180
                value: spot.modelData.angle ?? 0
                onMoved: points.tools.edit("point", {
                    "point": spot.number,
                    "angle": Math.round(value)
                }, true)
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: App.tr("Turn %1°", Math.round(value))
            }

            QQC2.ToolButton {
                visible: spot.index === points.list.length - 1 && spot.index > 0
                icon.name: "edit-delete"
                text: App.tr("Remove")
                display: QQC2.AbstractButton.IconOnly
                onClicked: points.tools.edit("point", {
                    "point": spot.number,
                    "remove": true
                })
            }
        }
    }
}
