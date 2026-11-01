import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: touch

    required property var owner
    readonly property var depth: owner.depth

    QQC2.Label {
        text: App.tr("Bones that take the pointer")
        font.bold: true
    }

    Repeater {
        model: touch.owner.touched

        delegate: SceneRigRow {
            required property var modelData
            required property int index

            text: modelData
            onRemoved: touch.owner.put("/hit", touch.owner.without(touch.owner.touched, index))
        }
    }

    RowLayout {
        QQC2.ComboBox {
            id: hit

            Layout.fillWidth: true
            model: touch.owner.names.filter(n => !touch.owner.touched.includes(n))
        }

        QQC2.Button {
            text: App.tr("Add")
            enabled: hit.currentText !== ""
            onClicked: touch.owner.put("/hit", touch.owner.touched.concat([hit.currentText]))
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: touch.owner.touched.length > 0
        wrapMode: Text.Wrap
        text: App.tr("Scripts learn which of these bones the pointer touched, with the pointer permission.")
    }

    QQC2.Switch {
        text: App.tr("Push vertices by their depth with perspective")
        checked: touch.depth !== null
        onToggled: touch.owner.put("/depth", checked ? {
            "scale": 1
        } : null)
    }

    RowLayout {
        visible: touch.depth !== null

        QQC2.Label {
            text: App.tr("Depth")
        }

        QQC2.Slider {
            Layout.fillWidth: true
            from: 0
            to: 10
            value: touch.depth?.scale ?? 1
            onMoved: touch.owner.put("/depth", Object.assign({}, touch.depth, {
                "scale": Math.round(value * 100) / 100
            }))
        }

        QQC2.Switch {
            text: App.tr("Hide parts seen from behind")
            checked: touch.depth?.cull === true
            onToggled: touch.owner.put("/depth", Object.assign({}, touch.depth, {
                "cull": checked
            }))
        }
    }
}
