import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

GridLayout {
    id: row

    required property var steer
    required property var keep
    readonly property bool ring: keep.kind === "keep_distance"
    readonly property var targets: steer.targets

    columns: 2

    QQC2.Label {
        Layout.columnSpan: 2
        text: row.ring ? App.tr("Keep a distance from") : App.tr("Keep between")
    }

    Repeater {
        model: row.ring ? [["target", row.keep.target]] : [["from", row.keep.from_ref], ["to", row.keep.to_ref]]

        delegate: QQC2.ComboBox {
            required property var modelData
            readonly property var value: modelData[1]

            Layout.fillWidth: true
            Layout.columnSpan: row.ring ? 2 : 1
            model: row.targets.concat(Array.isArray(value) ? [App.tr("a point")] : [])
            currentIndex: Array.isArray(value) ? row.targets.length : row.targets.indexOf(value)
            onActivated: index => {
                if (index < row.targets.length)
                    row.steer.patch(row.keep, v => v[modelData[0]] = row.targets[index]);
            }
        }
    }

    QQC2.Label {
        visible: row.ring
        text: App.tr("Distance %1 px", Math.round(row.keep.distance ?? 0))
    }

    QQC2.Slider {
        Layout.fillWidth: true
        visible: row.ring
        from: 0
        to: Math.max(1000, row.keep.distance ?? 0)
        value: row.keep.distance ?? 0
        onMoved: row.steer.patch(row.keep, v => v.distance = Math.round(value), true)
    }

    QQC2.Label {
        text: App.tr("Strength")
    }

    QQC2.Slider {
        Layout.fillWidth: true
        from: -50
        to: 50
        value: row.keep.strength ?? 0
        onMoved: row.steer.patch(row.keep, v => v.strength = Math.round(value * 10) / 10, true)
    }

    QQC2.Button {
        Layout.columnSpan: 2
        icon.name: "edit-delete"
        text: App.tr("Remove this force")
        onClicked: row.steer.tools.edit("force", {
                "index": row.keep.index,
                "remove": true
            })
    }
}
