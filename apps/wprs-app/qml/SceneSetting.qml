import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

QQC2.ItemDelegate {
    id: row

    required property var page
    required property var tools
    required property var modelData
    readonly property string kind: SceneWords.label(modelData.type)

    width: ListView.view.width
    leftPadding: Kirigami.Units.largeSpacing * (1 + 2 * modelData.depth)
    rightPadding: buttons.width
    highlighted: page.editor.selected === modelData.pointer
    text: modelData.label === kind ? kind : modelData.label + " · " + kind
    onClicked: page.editor.select(modelData.pointer)

    QQC2.ToolTip.text: modelData.id
    QQC2.ToolTip.visible: hovered
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    RowLayout {
        id: buttons

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 0

        QQC2.ToolButton {
            icon.name: "go-up"
            text: App.tr("Move up")
            display: QQC2.AbstractButton.IconOnly
            enabled: row.modelData.index > 0
            onClicked: row.tools.move(row.modelData, row.modelData.index - 1)
        }

        QQC2.ToolButton {
            icon.name: "go-down"
            text: App.tr("Move down")
            display: QQC2.AbstractButton.IconOnly
            enabled: row.modelData.index < row.modelData.count - 1
            onClicked: row.tools.move(row.modelData, row.modelData.index + 1)
        }

        QQC2.ToolButton {
            id: more

            icon.name: "overflow-menu"
            text: App.tr("More")
            display: QQC2.AbstractButton.IconOnly
            onClicked: row.tools.show(more, row.modelData)
        }
    }
}
