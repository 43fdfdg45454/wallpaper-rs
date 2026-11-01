import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

GridLayout {
    id: source

    required property var tools
    required property var emitter
    readonly property var layers: tools.info?.sources ?? []

    function set(change) {
        const args = {
            "emitter": emitter.index,
            "id": emitter.id,
            "threshold": emitter.threshold,
            "color": emitter.color === true,
            "inherit": emitter.inherit === true,
            "refresh": emitter.refresh
        };
        if (typeof emitter.jitter === "number")
            args.jitter = emitter.jitter;
        tools.edit("layer", Object.assign(args, change), change.threshold !== undefined);
    }

    columns: 2

    QQC2.Label {
        text: App.tr("Layer")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: source.layers
        currentIndex: source.layers.indexOf(source.emitter.id)
        onActivated: index => source.set({
                "id": source.layers[index]
            })
    }

    QQC2.Label {
        text: App.tr("Threshold")
    }

    QQC2.Slider {
        Layout.fillWidth: true
        from: 0
        to: 1
        stepSize: 0.01
        value: source.emitter.threshold ?? 0.5
        onMoved: source.set({
            "threshold": Math.round(value * 100) / 100
        })
        QQC2.ToolTip.visible: pressed
        QQC2.ToolTip.text: App.tr("Pixels more opaque than %1 emit; the preview marks them", value.toFixed(2))
    }

    QQC2.CheckBox {
        text: App.tr("Its color")
        checked: source.emitter.color === true
        onToggled: source.set({
            "color": checked
        })
    }

    QQC2.CheckBox {
        text: App.tr("Its motion")
        checked: source.emitter.inherit === true
        onToggled: source.set({
            "inherit": checked
        })
    }

    QQC2.Label {
        text: App.tr("Read again")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Once"), App.tr("When the layer changes")]
        currentIndex: source.emitter.refresh === "on_change" ? 1 : 0
        onActivated: index => source.set({
                "refresh": index ? "on_change" : "static"
            })
    }

    QQC2.Label {
        Layout.columnSpan: 2
        Layout.fillWidth: true
        visible: source.emitter.approximate === true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("The preview marks the box of this layer; the engine emits from its opaque pixels.")
    }
}
