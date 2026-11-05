import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tint

    required property var tools
    required property var init
    readonly property var modes: ["color", "hsv", "palette", "gradient"]
    readonly property string mode: init.hsv ? "hsv" : init.palette ? "palette" : (init.gradient ?? []).length > 0 ? "gradient" : "color"
    readonly property var pair: Array.isArray(init.color) && typeof init.color[0] === "string" ? init.color : ["#ffffff", "#ffffff"]
    readonly property var hues: Object.assign({
            "hue": [0, 360],
            "sat": [1, 1],
            "val": [1, 1]
        }, init.hsv ?? {})
    readonly property var colors: init.palette?.colors ?? []
    readonly property var stops: init.gradient ?? []

    function put(key, value, gesture) {
        tools.edit("start", {
                "ranges": {
                    [key]: value
                }
            }, gesture);
    }

    function choose(index) {
        const key = modes[index];
        if (key === "color")
            tools.edit("start", {
                    "clear": ["hsv", "palette", "gradient"]
                });
        else if (key === "hsv")
            put("hsv", hues, false);
        else if (key === "palette")
            put("palette", {
                    "colors": pair.slice()
                }, false);
        else
            put("gradient", [[0, pair[0]], [1, pair[1]]], false);
    }

    function hue(key, ends, gesture) {
        const h = Object.assign({}, hues);
        h[key] = ends;
        put("hsv", h, gesture);
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Starting colors")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("Between two colors"), App.tr("Hues, saturation and value"), App.tr("From a palette"), App.tr("Along a gradient")]
            currentIndex: tint.modes.indexOf(tint.mode)
            onActivated: index => tint.choose(index)
        }
    }

    RowLayout {
        visible: tint.mode === "color"

        Repeater {
            model: [0, 1]

            delegate: SceneColor {
                required property int modelData

                hex: tint.pair[modelData]
                onPicked: hex => {
                    const p = tint.pair.slice();
                    p[modelData] = hex;
                    tint.put("color", p, false);
                }
            }
        }
    }

    SceneSwarmHues {
        Layout.fillWidth: true
        visible: tint.mode === "hsv"
        tint: tint
    }

    SceneSwarmPalette {
        Layout.fillWidth: true
        visible: tint.mode === "palette"
        tint: tint
    }

    SceneGradientBar {
        objectName: "gradient"
        Layout.fillWidth: true
        visible: tint.mode === "gradient"
        stops: tint.stops
        onEdited: (stops, gesture) => tint.put("gradient", stops, gesture)
    }
}
