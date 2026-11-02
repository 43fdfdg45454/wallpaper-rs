import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    property var info: null
    property var cost: null

    function ask(q) {
        return JSON.parse(page.editor.rig("", JSON.stringify(q)) || "null");
    }

    function load() {
        if (!visible)
            return;
        info = ask({
            "lights": {
                "time": page.editor.time
            }
        });
        cost = ask({
            "lighting": {
                "time": page.editor.time,
                "quality": SceneLook.quality
            }
        });
    }

    function parts(row) {
        const out = [];
        if ((cost?.volumetric ?? 0) > 0)
            out.push(row.air ? App.tr("air at %1 of the size, %2 taps", row.air.scale === 0.5 ? App.tr("half") : App.tr("a quarter"), Math.round(row.air_taps)) : App.tr("no air"));
        if ((cost?.lit_systems ?? []).length > 0)
            out.push(App.tr("%1 lit particle lights", Math.round(row.lit_particles)));
        if ((cost?.refracting ?? []).length > 0)
            out.push(row.refract === null ? App.tr("no refraction") : App.tr("refraction over %1 pixels", Math.round(row.refract_pixels)));
        return out.join(", ");
    }

    function extra(row) {
        const more = parts(row);
        return row.quality.toUpperCase() + (more ? ": " + more : "");
    }

    function level(row) {
        const base = row.baked ? App.tr("%1: lighting worked out once as the scene starts; nothing more each frame", row.quality.toUpperCase()) : App.tr("%1: %2 lights over %3 lit layers, %4 pixel lights each frame", row.quality.toUpperCase(), row.lights, row.lit_layers, Math.round(row.per_frame));
        const more = parts(row);
        return more ? base + "; " + more : base;
    }

    onVisibleChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            panel.load();
        }

        target: panel.page.editor
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Lights")
    }

    Repeater {
        model: panel.info?.lights ?? []

        delegate: RowLayout {
            required property var modelData

            Layout.fillWidth: true

            QQC2.ToolButton {
                Layout.fillWidth: true
                text: [modelData.id, SceneWords.label(modelData.type, "")].concat(modelData.volumetric ? [App.tr("air")] : []).concat(modelData.texture ? [App.tr("projects")] : []).join(" · ")
                onClicked: panel.page.editor.select(modelData.pointer)
            }

            QQC2.ToolButton {
                icon.name: "media-record"
                text: App.tr("Solo")
                checkable: true
                checked: modelData.solo
                onToggled: panel.page.run("light", {
                    "solo": {
                        "layer": checked ? modelData.id : null
                    }
                })
            }

            QQC2.ToolButton {
                icon.name: "audio-volume-muted"
                text: App.tr("Mute")
                checkable: true
                checked: modelData.muted
                onToggled: panel.page.run("light", {
                    "mute": {
                        "layer": modelData.id,
                        "muted": checked
                    }
                })
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: App.tr("%1 of at most %2 lights", panel.info?.count ?? 0, panel.info?.max ?? 12)
    }

    Repeater {
        model: panel.info?.tiers ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: modelData.left_out > 0 ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
            text: App.tr("%1: %2 of %3 lights, %4 left out", modelData.tier.toUpperCase(), modelData.kept, modelData.cap, modelData.left_out)
        }
    }

    SceneAmbient {
        Layout.fillWidth: true
        page: panel.page
    }

    SceneQuality {
        page: panel.page
        again: App.tr("Restart the preview")
        onPicked: panel.load()
    }

    Repeater {
        model: panel.cost?.levels ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            font.bold: modelData.quality === SceneLook.quality
            text: panel.level(modelData)
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: (panel.cost?.volumetric ?? 0) > 0 || (panel.cost?.lit_systems ?? []).length > 0 || (panel.cost?.refracting ?? []).length > 0
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("On phones: %1", (panel.cost?.phone ?? []).map(row => panel.extra(row)).join(" · "))
    }
}
