import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: pairs

    required property var host
    readonly property var rows: Array.isArray(host.value) ? host.value : []
    readonly property bool stops: host.field.widget === "stops"

    function put(row, column, value) {
        const next = rows.map(pair => pair.slice());
        next[row][column] = value;
        host.commit(next);
    }

    function number(value) {
        return String(Number(Number(value).toFixed(4)));
    }

    Repeater {
        model: pairs.rows

        delegate: RowLayout {
            id: pair

            required property var modelData
            required property int index

            Layout.fillWidth: true

            QQC2.TextField {
                Layout.fillWidth: true
                text: pairs.number(pair.modelData[0])
                onEditingFinished: pairs.put(pair.index, 0, pairs.host.parse(text, pair.modelData[0]))
            }

            QQC2.TextField {
                Layout.fillWidth: true
                visible: !pairs.stops
                text: pairs.number(pair.modelData[1])
                onEditingFinished: pairs.put(pair.index, 1, pairs.host.parse(text, pair.modelData[1]))
            }

            SceneColor {
                visible: pairs.stops
                hex: typeof pair.modelData[1] === "string" ? pair.modelData[1] : "#ffffff"
                onPicked: hex => pairs.put(pair.index, 1, hex)
            }

            QQC2.ToolButton {
                visible: pairs.stops
                enabled: pairs.rows.length > 2
                icon.name: "list-remove"
                text: App.tr("Remove")
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                onClicked: pairs.host.commit(pairs.rows.filter((_, other) => other !== pair.index))
            }
        }
    }

    QQC2.Button {
        visible: pairs.stops
        icon.name: "list-add"
        text: App.tr("Add")
        onClicked: pairs.host.commit(pairs.rows.concat([[1, (pairs.rows[pairs.rows.length - 1] ?? [1, "#ffffff"])[1]]]))
    }
}
