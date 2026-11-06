import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.FormLayout {
    id: look

    required property var page
    readonly property var bloom: page.view.canvas?.bloom ?? null
    readonly property var glide: page.view.canvas?.parallax ?? null
    readonly property var glows: [["threshold", App.tr("Threshold"), 0, 4, 0.8], ["strength", App.tr("Strength"), 0, 8, 1], ["radius", App.tr("Radius"), 0, 128, 16]]
    readonly property var moves: [["amount", App.tr("Depth"), 0, 4, 1], ["delay", App.tr("Delay"), 0, 2, 0.15], ["pointer", App.tr("Pointer"), 0, 1, 1]]

    function edit(what, args) {
        page.run("canvas", {
            [what]: args
        });
    }

    function slid(what, key, value, fallback) {
        const stored = (what === "bloom" ? bloom : glide)?.[key];
        const rounded = Math.round(value * 100) / 100;
        if (rounded !== (typeof stored === "number" ? stored : fallback))
            page.run("canvas", {
                [what]: {
                    [key]: rounded
                }
            }, true);
    }

    QQC2.Switch {
        Kirigami.FormData.label: App.tr("Glow:")
        text: App.tr("Bright parts glow")
        checked: look.bloom !== null
        onToggled: look.edit("bloom", checked ? {} : {
            "off": true
        })
    }

    Repeater {
        model: look.glows

        delegate: QQC2.Slider {
            required property var modelData

            visible: look.bloom !== null
            Kirigami.FormData.label: modelData[1] + ":"
            from: modelData[2]
            to: modelData[3]
            stepSize: (modelData[3] - modelData[2]) / 100
            value: typeof look.bloom?.[modelData[0]] === "number" ? look.bloom[modelData[0]] : modelData[4]
            onMoved: look.slid("bloom", modelData[0], value, modelData[4])
        }
    }

    QQC2.Switch {
        Kirigami.FormData.label: App.tr("Parallax:")
        text: App.tr("Layers move in depth")
        checked: look.glide !== null
        onToggled: look.edit("parallax", checked ? {} : {
            "off": true
        })
    }

    Repeater {
        model: look.moves

        delegate: QQC2.Slider {
            required property var modelData

            visible: look.glide !== null
            Kirigami.FormData.label: modelData[1] + ":"
            from: modelData[2]
            to: modelData[3]
            stepSize: (modelData[3] - modelData[2]) / 100
            value: typeof look.glide?.[modelData[0]] === "number" ? look.glide[modelData[0]] : modelData[4]
            onMoved: look.slid("parallax", modelData[0], value, modelData[4])
        }
    }

    QQC2.CheckBox {
        visible: look.glide !== null
        text: App.tr("Follow the tilt of a phone")
        checked: look.glide?.gyro === true
        onToggled: look.edit("parallax", {
            "gyro": checked
        })
    }
}
