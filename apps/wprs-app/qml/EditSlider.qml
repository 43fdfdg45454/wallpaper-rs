import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2

ColumnLayout {
    id: row

    property string text
    property real from: 0
    property real to: 1
    property real stepSize: 0.01
    property real value
    property int decimals: stepSize < 1 ? 2 : 0

    signal moved(real value)

    Layout.fillWidth: true
    spacing: 0

    RowLayout {
        QQC2.Label {
            Layout.fillWidth: true
            text: row.text
        }

        QQC2.Label {
            text: slider.value.toFixed(row.decimals)
        }
    }

    QQC2.Slider {
        id: slider

        Layout.fillWidth: true
        from: row.from
        to: row.to
        stepSize: row.stepSize
        value: row.value
        onMoved: row.moved(value)
    }
}
