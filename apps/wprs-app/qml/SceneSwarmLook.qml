import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

GridLayout {
    id: look

    required property var tools
    readonly property var cut: tools.info?.cutout ?? null
    readonly property var turn: tools.info?.orient ?? ({})
    readonly property var focal: tools.info?.perspective ?? null
    readonly property var depth: tools.info?.depth ?? ({})
    readonly property var modes: ["screen", "velocity", "fixed"]

    columns: 2

    Kirigami.Heading {
        Layout.columnSpan: 2
        level: 4
        text: App.tr("Look")
    }

    QQC2.CheckBox {
        Layout.columnSpan: 2
        text: App.tr("Cut out the alpha of the texture")
        checked: look.cut !== null
        onToggled: look.tools.edit("cutout", checked ? {
            "range": [0.3, 0.7, 1]
        } : {})
    }

    QQC2.RangeSlider {
        id: cutRange

        Layout.columnSpan: 2
        Layout.fillWidth: true
        visible: look.cut !== null
        from: 0
        to: 1
        first.value: look.cut?.start ?? 0.3
        second.value: look.cut?.end ?? 0.7
        first.onMoved: look.tools.edit("cutout", {
            "range": [first.value, Math.max(first.value + 0.01, second.value), look.cut.opacity].map(v => Math.round(v * 100) / 100)
        }, true)
        second.onMoved: look.tools.edit("cutout", {
            "range": [Math.min(first.value, second.value - 0.01), second.value, look.cut.opacity].map(v => Math.round(v * 100) / 100)
        }, true)
    }

    QQC2.Label {
        visible: look.cut !== null
        text: App.tr("Opacity")
    }

    QQC2.Slider {
        Layout.fillWidth: true
        visible: look.cut !== null
        from: 0
        to: 1
        value: look.cut?.opacity ?? 1
        onMoved: look.tools.edit("cutout", {
            "range": [look.cut.start, look.cut.end, Math.round(value * 100) / 100]
        }, true)
    }

    SceneCurvePlot {
        Layout.columnSpan: 2
        Layout.fillWidth: true
        visible: look.cut !== null
        curve: look.cut?.ramp ?? null
        marks: look.cut ? [look.cut.start, look.cut.end] : []
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Face the screen"), App.tr("Along the motion"), App.tr("At a fixed angle")]
        currentIndex: Math.max(0, look.modes.indexOf(look.turn.mode ?? "screen"))
        onActivated: index => look.tools.edit("orient", {
                "orient": look.modes[index],
                "angle": look.turn.angle ?? 0
            })
    }

    QQC2.Dial {
        enabled: look.turn.mode === "fixed"
        implicitWidth: Kirigami.Units.gridUnit * 2.5
        implicitHeight: implicitWidth
        from: -180
        to: 180
        value: look.turn.angle ?? 0
        onMoved: look.tools.edit("orient", {
            "orient": "fixed",
            "angle": Math.round(value)
        }, true)
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.text: App.tr("Turn %1°", Math.round(value))
    }

    SceneSwarmDepth {
        Layout.columnSpan: 2
        Layout.fillWidth: true
        tools: look.tools
        focal: look.focal
        depth: look.depth
    }
}
