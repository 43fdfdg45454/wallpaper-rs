import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tones

    required property var page
    readonly property var maps: ["none", "aces", "agx"]
    readonly property bool hdr: page.editor.value("/scene/canvas/hdr") === "true"
    readonly property string map: JSON.parse(page.editor.value("/scene/canvas/tonemap") || "null") ?? "none"
    readonly property real split: Number(JSON.parse(page.editor.value("/scene/ext/editor/tonemap_split") || "0"))

    function set(args) {
        page.run("scene3d", {
            "tones": args
        });
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Bright colors")
    }

    QQC2.Switch {
        text: App.tr("Keep colors brighter than white")
        checked: tones.hdr
        onToggled: tones.set(checked ? {
            "hdr": true
        } : {
            "hdr": false,
            "tonemap": "none"
        })
    }

    RowLayout {
        enabled: tones.hdr

        QQC2.Label {
            text: App.tr("Tone map")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("None"), App.tr("Filmic (ACES)"), App.tr("Soft (AgX)")]
            currentIndex: Math.max(0, tones.maps.indexOf(tones.map))
            onActivated: index => tones.set({
                    "tonemap": tones.maps[index]
                })
        }
    }

    RowLayout {
        enabled: tones.hdr && tones.map !== "none"

        QQC2.Label {
            text: App.tr("Before and after")
        }

        SceneSlider {
            Layout.fillWidth: true
            from: 0
            to: 1
            stepSize: 0.05
            value: tones.split
            onMoved: tones.set({
                "split": value
            })
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("The preview shows the left part without the tone map. Phones keep 8-bit colors before the tone map.")
    }

    QQC2.Label {
        readonly property var lit: Object.keys(App.state.outputs ?? {}).filter(name => App.state.outputs[name].hdr?.active)

        Layout.fillWidth: true
        visible: tones.hdr
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        font: Kirigami.Theme.smallFont
        text: App.tr("On HDR screens the colors brighter than white are kept and bent softly to how bright each screen gets, instead of this tone map; the preview here shows the standard picture.") + (lit.length > 0 ? " " + App.tr("HDR is on for: %1", lit.join(", ")) : "")
    }
}
