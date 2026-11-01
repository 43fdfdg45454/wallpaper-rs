import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: fields

    required property var tools
    readonly property var gust: tools.info?.fields?.turbulent?.turbulent ?? null
    readonly property var drift: tools.info?.fields?.offset?.offset ?? null
    readonly property var bands: ["", "bass", "mid", "treble", "level", "beat"]
    readonly property var signs: [0, 1, -1]

    function set(what, key, value) {
        tools.edit(what, {
                [what]: {
                    [key]: value
                }
            }, true);
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Starting directions and offsets")
    }

    QQC2.CheckBox {
        text: App.tr("Turn starting directions with a noise")
        checked: fields.gust !== null
        onToggled: fields.tools.edit("turbulent", checked ? {
                "turbulent": {}
            } : {
                "off": true
            })
    }

    SceneSwarmSliders {
        Layout.fillWidth: true
        visible: fields.gust !== null
        value: fields.gust ?? ({})
        rows: [["forward", App.tr("Direction"), -180, 180, 1], ["scale", App.tr("Turn by up to"), 0, 1, 0.01], ["noise_speed", App.tr("Noise speed"), 0, 10, 0.1], ["offset", App.tr("Spread between particles"), 0, 20, 0.1], ["phase", App.tr("Phase"), 0, 10, 0.1]]
        onEdited: (key, v) => fields.set("turbulent", key, v)
    }

    RowLayout {
        visible: fields.gust !== null

        QQC2.Label {
            text: App.tr("Speed")
        }

        QQC2.RangeSlider {
            id: speed

            Layout.fillWidth: true
            from: 0
            to: 1000
            first.value: fields.gust?.speed?.[0] ?? 10
            second.value: fields.gust?.speed?.[1] ?? 40
            first.onMoved: fields.set("turbulent", "speed", [Math.round(first.value), Math.round(second.value)])
            second.onMoved: fields.set("turbulent", "speed", [Math.round(first.value), Math.round(second.value)])
        }
    }

    RowLayout {
        visible: fields.gust !== null

        QQC2.Label {
            text: App.tr("Sound multiplies the phase")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("No sound"), App.tr("Bass"), App.tr("Mid"), App.tr("Treble"), App.tr("Level"), App.tr("Beat")]
            currentIndex: Math.max(0, fields.bands.indexOf(fields.gust?.audio ?? ""))
            onActivated: index => fields.tools.edit("turbulent", index === 0 ? {
                        "quiet": true
                    } : {
                        "turbulent": {
                            "audio": fields.bands[index]
                        }
                    })
        }
    }

    QQC2.CheckBox {
        text: App.tr("Move births along a fractal noise")
        checked: fields.drift !== null
        onToggled: fields.tools.edit("offset", checked ? {
                "offset": {}
            } : {
                "off": true
            })
    }

    SceneSwarmSliders {
        Layout.fillWidth: true
        visible: fields.drift !== null
        value: fields.drift ?? ({})
        rows: [["distance", App.tr("Distance"), 0, 200, 1], ["scale", App.tr("Pattern size"), 0, 10, 0.1], ["octaves", App.tr("Detail"), 1, 6, 1], ["noise_scale", App.tr("Scatter"), 0, 10, 0.1], ["noise_speed", App.tr("Drift"), 0, 10, 0.1]]
        onEdited: (key, v) => fields.set("offset", key, v)
    }

    Repeater {
        model: fields.drift ? [[0, App.tr("Across")], [1, App.tr("Down")]] : []

        delegate: RowLayout {
            required property var modelData

            QQC2.Label {
                text: modelData[1]
            }

            QQC2.ComboBox {
                Layout.fillWidth: true
                model: [App.tr("Both sides"), App.tr("Positive side"), App.tr("Negative side")]
                currentIndex: Math.max(0, fields.signs.indexOf(fields.drift.sign?.[modelData[0]] ?? 0))
                onActivated: index => {
                    const sign = (fields.drift.sign ?? [0, 0]).slice();
                    sign[modelData[0]] = fields.signs[index];
                    fields.set("offset", "sign", sign);
                }
            }
        }
    }
}
