import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2

GridLayout {
    id: sliders

    required property var value
    required property var rows

    signal edited(string key, real value)

    columns: 3

    SceneRepeater {
        id: listed3

        rows: sliders.rows

        delegate: QQC2.Label {
            readonly property var modelData: listed3.rows[index]
            required property int index

            Layout.row: index
            Layout.column: 0
            text: modelData[1]
        }
    }

    SceneRepeater {
        id: listed2

        rows: sliders.rows

        delegate: SceneSlider {
            readonly property var modelData: listed2.rows[index]
            required property int index

            Layout.row: index
            Layout.column: 1
            Layout.fillWidth: true
            from: modelData[2]
            to: modelData[3]
            stepSize: modelData[4]
            value: sliders.value[modelData[0]] ?? 0
            onMoved: sliders.edited(modelData[0], Number((Math.round(value / modelData[4]) * modelData[4]).toFixed(2)))
        }
    }

    SceneRepeater {
        id: listed

        rows: sliders.rows

        delegate: QQC2.Label {
            readonly property var modelData: listed.rows[index]
            required property int index

            Layout.row: index
            Layout.column: 2
            text: Number(sliders.value[modelData[0]] ?? 0).toLocaleString(Qt.locale(), "f", modelData[4] < 1 ? 2 : 0)
        }
    }
}
