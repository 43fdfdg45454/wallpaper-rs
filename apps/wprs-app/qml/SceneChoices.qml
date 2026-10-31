import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

QQC2.ComboBox {
    id: combo

    required property var modelData

    signal edited(string key, var value)

    Accessible.name: combo.modelData.label
    model: combo.modelData.options
    textRole: "label"
    valueRole: "value"
    currentIndex: (combo.modelData.options ?? []).findIndex(option => option.value === combo.modelData.value)
    onActivated: combo.edited(combo.modelData.key, currentValue)

    delegate: QQC2.ItemDelegate {
        id: choice

        required property var modelData
        required property int index

        width: ListView.view?.width ?? implicitWidth
        text: modelData.label
        icon.name: modelData.icon ?? ""
        highlighted: combo.highlightedIndex === index

        contentItem: RowLayout {
            Kirigami.Icon {
                visible: choice.icon.name !== ""
                source: choice.icon.name
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: implicitWidth
            }

            QQC2.Label {
                Layout.fillWidth: true
                text: choice.text
                color: choice.highlighted ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                textFormat: Text.PlainText
                elide: Text.ElideRight
            }
        }
    }
}
