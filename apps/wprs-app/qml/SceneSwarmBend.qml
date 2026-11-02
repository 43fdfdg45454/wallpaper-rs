import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: bend

    required property var light
    readonly property var info: light.info
    readonly property var bent: info?.refract ?? null
    readonly property var page: light.tools.page

    function put(args, gesture) {
        SceneSwarmGlow.run(page, "refract", args, gesture);
    }

    QQC2.CheckBox {
        text: App.tr("Bend what is behind the particles")
        checked: bend.bent !== null
        enabled: (bend.info?.pictures ?? []).length > 0
        onToggled: bend.put(checked ? {
            "normal": bend.info.normal ?? bend.info.pictures[0],
            "amount": 0.3
        } : {
            "off": true
        })
    }

    RowLayout {
        visible: bend.bent !== null

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: bend.info?.pictures ?? []
            currentIndex: Math.max(0, (bend.info?.pictures ?? []).indexOf(bend.bent?.normal ?? ""))
            onActivated: index => bend.put({
                    "normal": bend.info.pictures[index]
                })
        }

        QQC2.Slider {
            Layout.fillWidth: true
            from: 0
            to: 1
            value: bend.bent?.amount ?? 0.3
            onMoved: bend.put({
                "amount": Math.round(value * 100) / 100
            }, true)
        }
    }

    Image {
        visible: bend.bent !== null && !!bend.info?.field
        source: bend.info?.field ?? ""
        sourceSize.width: 48
        sourceSize.height: 48
    }

    QQC2.CheckBox {
        visible: bend.bent !== null
        text: App.tr("Show the displacement over the preview")
        checked: SceneSwarmGlow.shown
        onToggled: {
            SceneSwarmGlow.shown = checked;
            SceneSwarmCensus.shown = checked || SceneSwarmCensus.shown;
            if (checked)
                SceneSwarmCensus.measure(bend.page, bend.light.tools.system, null);
        }
    }

    Repeater {
        model: bend.bent !== null ? bend.info.levels : []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            font.bold: modelData.quality === SceneLook.quality
            text: modelData.refract === null ? App.tr("%1: left out", modelData.quality.toUpperCase()) : modelData.refract === 1 ? App.tr("%1: shifts up to %2 px at full size", modelData.quality.toUpperCase(), Math.round(bend.info.shift)) : App.tr("%1: shifts up to %2 px at half size", modelData.quality.toUpperCase(), Math.round(bend.info.shift))
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: bend.bent !== null
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Refracting particles are drawn only as the bend of what is behind them; Q1 and Q0, where phones often run, leave them out. Draw the system with a second layer to paint them too.")
    }
}
