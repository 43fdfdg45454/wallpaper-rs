import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

GridLayout {
    id: row

    required property var tools
    required property var wall
    required property var names
    required property var kinds
    readonly property var responses: ["bounce", "slide", "stop", "stick", "kill"]
    readonly property var sources: tools.info?.sources ?? []

    function patch(change) {
        const v = JSON.parse(JSON.stringify(wall.value));
        change(v);
        tools.edit("collider", {
                "index": wall.index,
                "collider": v
            });
    }

    function respond(response, more, gesture) {
        tools.edit("response", Object.assign({
                    "index": wall.index,
                    "response": response
                }, more ?? {}), gesture);
    }

    columns: 2

    QQC2.Label {
        text: App.tr("%1 %2", wall.index + 1, row.names[row.kinds.indexOf(wall.kind)] ?? wall.kind)
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Bounce"), App.tr("Slide"), App.tr("Stop"), App.tr("Stick"), App.tr("Kill")]
        currentIndex: row.responses.indexOf(row.wall.response)
        onActivated: index => row.respond(row.responses[index])
    }

    QQC2.CheckBox {
        Layout.columnSpan: 2
        visible: row.wall.kind === "circle" || row.wall.kind === "box"
        text: App.tr("Keep particles inside")
        checked: row.wall.inside
        onToggled: row.patch(v => v.bounds[row.wall.kind].inside = checked)
    }

    QQC2.Label {
        visible: row.wall.kind === "layer"
        text: App.tr("Layer")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        visible: row.wall.kind === "layer"
        model: row.sources
        currentIndex: row.sources.indexOf(row.wall.id)
        onActivated: index => row.patch(v => v.bounds.layer.id = row.sources[index])
    }

    QQC2.Label {
        visible: row.wall.response === "bounce"
        text: App.tr("Bounce")
    }

    QQC2.Slider {
        Layout.fillWidth: true
        visible: row.wall.response === "bounce"
        from: 0.01
        to: 1
        value: row.wall.bounce
        onMoved: row.respond("bounce", {
                "bounce": Math.round(value * 100) / 100
            }, true)
    }

    QQC2.Label {
        visible: row.wall.response === "bounce" || row.wall.response === "slide"
        text: App.tr("Friction")
    }

    QQC2.Slider {
        Layout.fillWidth: true
        visible: row.wall.response === "bounce" || row.wall.response === "slide"
        from: 0
        to: 1
        value: row.wall.friction
        onMoved: row.respond(row.wall.response, {
                "friction": Math.round(value * 100) / 100
            }, true)
    }

    QQC2.Button {
        Layout.columnSpan: 2
        icon.name: "edit-delete"
        text: App.tr("Remove this collider")
        onClicked: row.tools.edit("collider", {
                "index": row.wall.index,
                "remove": true
            })
    }
}
