import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: budget

    required property var tools
    readonly property var census: SceneSwarmCensus.census?.system === tools.system ? SceneSwarmCensus.census : null
    readonly property var glow: {
        const g = SceneSwarmGlow.info?.system === tools.system ? SceneSwarmGlow.info : null;
        return g && (g.lit || g.refract) ? g : null;
    }

    function measure() {
        SceneSwarmCensus.measure(tools.page, tools.system, null);
    }

    Connections {
        function onViewChanged() {
            if (SceneSwarmCensus.shown)
                again.restart();
        }

        target: budget.tools.page.editor
    }

    Timer {
        id: again

        interval: 300
        onTriggered: budget.measure()
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Particle budget")
    }

    SceneQuality {
        page: budget.tools.page
        again: App.tr("Restart systems")
        onPicked: budget.measure()
    }

    QQC2.CheckBox {
        text: App.tr("Simulate on the processor and show the particles over the preview")
        checked: SceneSwarmCensus.shown
        onToggled: {
            SceneSwarmCensus.shown = checked;
            if (checked)
                budget.measure();
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: budget.census !== null
        wrapMode: Text.Wrap
        text: App.tr("%1 alive of %2 at %3 s; %4 slots on the processor", budget.census?.alive ?? 0, budget.census?.capacity ?? 0, Math.round((budget.census?.time ?? 0) * 10) / 10, budget.census?.slots ?? 0)
    }

    Repeater {
        model: budget.census?.levels ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: App.tr("%1: %2 % of births, %3 neighbours, %4 trail points, %5 rope points between particles", modelData.quality.toUpperCase(), Math.round(modelData.births * 100), modelData.neighbours, modelData.trail, modelData.subdivide)
        }
    }

    Repeater {
        model: budget.census?.tiers ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: App.tr("%1: %2 slots on the GPU, %3 on the processor, %4 trail points, %5 neighbours, %6 rope segments", modelData.tier.toUpperCase(), modelData.gpu, modelData.cpu, modelData.trail, modelData.neighbours, modelData.rope)
        }
    }

    Repeater {
        model: glow ? glow.levels : []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: App.tr("%1: lit by up to %2 lights; refraction %3", modelData.quality.toUpperCase(), modelData.lights, modelData.refract === null ? App.tr("off") : modelData.refract === 1 ? App.tr("at full size") : App.tr("at half size"))
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: budget.census !== null
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Phones and older computers fall on T0 or T1, which keep fewer slots, neighbours, trail points and rope segments, light particles with up to 8 lights and refract only from Q2; software rendering runs on the processor.")
    }
}
