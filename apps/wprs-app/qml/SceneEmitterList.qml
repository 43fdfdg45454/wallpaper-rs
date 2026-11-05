import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: list

    required property var host
    readonly property var rows: Array.isArray(host.value) ? host.value : []

    function put(index, key, value, gesture) {
        const next = rows.map(row => Object.assign({}, row));
        if (value === undefined)
            delete next[index][key];
        else
            next[index][key] = value;
        host.commit(next, gesture);
    }

    function add(line) {
        const row = line ? {
            "from": [0.3, 1],
            "to": [0.7, 1],
            "angle": -90,
            "strength": 4
        } : {
            "from": [0.5, 0.9],
            "angle": -90,
            "strength": 4
        };
        host.commit(rows.concat([row]));
    }

    RowLayout {
        QQC2.Button {
            text: App.tr("Add a point emitter")
            icon.name: "draw-dot"
            enabled: list.rows.length < 64
            onClicked: list.add(false)
        }

        QQC2.Button {
            text: App.tr("Add a line emitter")
            icon.name: "draw-line"
            enabled: list.rows.length < 64
            onClicked: list.add(true)
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: list.rows.length > 0
        text: App.tr("Drag the emitters on the canvas to place them")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }

    Repeater {
        model: list.rows

        delegate: RowLayout {
            id: row

            required property var modelData
            required property int index

            QQC2.Label {
                text: row.modelData.to !== undefined ? App.tr("Line %1", row.index + 1) : App.tr("Point %1", row.index + 1)
            }

            QQC2.Dial {
                implicitWidth: Kirigami.Units.gridUnit * 2
                implicitHeight: implicitWidth
                from: -180
                to: 180
                value: row.modelData.angle ?? -90
                onMoved: list.put(row.index, "angle", Math.round(value), true)
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: App.tr("Direction %1°", Math.round(value))
            }

            QQC2.Slider {
                Layout.fillWidth: true
                from: 0
                to: 64
                value: row.modelData.strength ?? 0
                onMoved: list.put(row.index, "strength", Math.round(value * 10) / 10, true)
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: App.tr("Strength %1", Math.round(value * 10) / 10)
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: App.tr("Remove")
                display: QQC2.AbstractButton.IconOnly
                onClicked: list.host.commit(list.rows.filter((_, i) => i !== row.index))
            }
        }
    }
}
