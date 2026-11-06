import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Flow {
    id: swatches

    required property var field
    required property string current
    readonly property var chosen: current.split(",").filter(item => item !== "")

    signal picked(string value)

    Layout.preferredWidth: Kirigami.Units.gridUnit * 24
    spacing: Kirigami.Units.smallSpacing

    Repeater {
        model: swatches.field.values

        delegate: QQC2.AbstractButton {
            id: swatch

            required property string modelData

            implicitWidth: Kirigami.Units.gridUnit * 1.6
            implicitHeight: Kirigami.Units.gridUnit * 1.6
            checkable: true
            checked: swatches.chosen.includes(modelData)
            Accessible.name: "#" + modelData
            QQC2.ToolTip.text: "#" + modelData
            QQC2.ToolTip.visible: hovered
            onClicked: swatches.picked(App.backend.toggle(swatches.current, modelData, swatches.field.values.join(",")))

            background: Rectangle {
                color: "#" + swatch.modelData
                radius: Kirigami.Units.smallSpacing
                border.width: swatch.checked ? 3 : 1
                border.color: swatch.checked ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
            }
        }
    }
}
