import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: brush

    required property var page
    required property string kind
    required property var counted
    property real radius: 0.05
    property real falloff: 0.5
    property real strength: 0.2
    property bool second
    readonly property bool active: page.paint?.kind === kind
    readonly property bool fill: kind === "fill"
    readonly property var spec: ({
            "kind": kind,
            "radius": radius,
            "falloff": falloff,
            "by": strength,
            "second": second
        })

    function use(on) {
        page.paint = on ? spec : null;
    }

    onSpecChanged: {
        if (active)
            page.paint = spec;
    }

    QQC2.Button {
        Layout.fillWidth: true
        checkable: true
        checked: brush.active
        icon.name: "draw-brush"
        text: brush.fill ? App.tr("Paint fill marks") : App.tr("Paint depth")
        onClicked: brush.use(checked)
    }

    RowLayout {
        QQC2.RadioButton {
            checked: !brush.second
            text: brush.fill ? App.tr("Refill") : App.tr("Raise")
            onClicked: brush.second = false
        }

        QQC2.RadioButton {
            checked: brush.second
            text: brush.fill ? App.tr("Protect") : App.tr("Lower")
            onClicked: brush.second = true
        }
    }

    EditSlider {
        text: App.tr("Brush size")
        from: 0.005
        to: 0.3
        stepSize: 0.005
        decimals: 3
        value: brush.radius
        onMoved: value => brush.radius = value
    }

    EditSlider {
        visible: !brush.fill
        text: App.tr("Strength")
        from: 0.01
        value: brush.strength
        onMoved: value => brush.strength = value
    }

    EditSlider {
        text: App.tr("Soft edge")
        value: brush.falloff
        onMoved: value => brush.falloff = value
    }

    RowLayout {
        QQC2.Label {
            Layout.fillWidth: true
            text: brush.fill ? App.tr("%1 marks", brush.counted) : App.tr("%1 strokes", brush.counted)
        }

        QQC2.ToolButton {
            icon.name: "edit-undo"
            text: App.tr("Undo")
            enabled: brush.page.editor.canUndo
            onClicked: brush.page.editor.undo()
        }

        QQC2.ToolButton {
            icon.name: "edit-clear"
            text: brush.fill ? App.tr("Remove the marks") : App.tr("Remove the strokes")
            enabled: brush.counted > 0
            onClicked: brush.page.run(brush.fill ? "fill_mask" : "depth_brush", {
                    "clear": true
                })
        }
    }
}
