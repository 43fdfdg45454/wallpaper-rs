import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2

ColumnLayout {
    id: strand

    required property var strands
    required property string strand
    required property var value
    required property var fields
    required property var toggles

    Repeater {
        model: strand.fields

        delegate: RowLayout {
            required property var modelData

            Layout.fillWidth: true

            QQC2.Label {
                text: modelData[1]
            }

            QQC2.Slider {
                Layout.fillWidth: true
                from: modelData[2]
                to: modelData[3]
                stepSize: modelData[5]
                value: strand.value[modelData[0]] ?? modelData[4]
                onMoved: strand.strands.set(strand.strand, {
                        [modelData[0]]: Math.round(value / modelData[5]) * modelData[5]
                    }, true)
            }
        }
    }

    Repeater {
        model: strand.toggles

        delegate: QQC2.CheckBox {
            required property var modelData

            text: modelData[1]
            checked: strand.value[modelData[0]] ?? modelData[2]
            onToggled: strand.strands.set(strand.strand, {
                    [modelData[0]]: checked
                })
        }
    }
}
