import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tools

    required property var page
    required property var node
    readonly property string lamp: node?.type === "light" ? node.id : ""
    readonly property var types: ["point", "spot", "tube", "directional"]
    property var light: null
    property var falloff: null

    function load() {
        if (!lamp) {
            light = null;
            return;
        }
        const all = JSON.parse(page.editor.rig("", JSON.stringify({
            "lights": {
                "time": page.editor.time
            }
        })) || "null");
        light = (all?.lights ?? []).find(l => l.id === lamp) ?? null;
        falloff = JSON.parse(page.editor.rig(lamp, JSON.stringify({
            "falloff": {
                "layer": lamp,
                "time": page.editor.time
            }
        })) || "null");
    }

    function set(args) {
        page.run("light", {
            "set": Object.assign({
                "layer": lamp
            }, args)
        }, true);
    }

    visible: light !== null
    onLampChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            tools.load();
        }

        target: tools.page.editor
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Light")
    }

    RowLayout {
        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("Point"), App.tr("Spot"), App.tr("Tube"), App.tr("Directional")]
            currentIndex: tools.types.indexOf(tools.light?.type ?? "point")
            onActivated: index => tools.set({
                    "light": tools.types[index]
                })
        }

        SceneColor {
            alpha: false
            hex: {
                const v = tools.node ? JSON.parse(tools.page.editor.value(tools.node.pointer + "/color") || "null") : null;
                return typeof v === "string" ? v : "#ffffff";
            }
            onPicked: hex => tools.page.change(tools.node.pointer + "/color", hex)
        }
    }

    SceneSliders {
        Layout.fillWidth: true
        value: tools.light ? {
            "intensity": tools.light.intensity,
            "radius": tools.light.radius,
            "z": tools.light.z,
            "length": tools.light.length ?? 0,
            "inner": tools.light.cone?.[0] ?? 0,
            "outer": tools.light.cone?.[1] ?? 0
        } : {}
        rows: [["intensity", App.tr("Intensity"), 0, 10, 0.05], ["radius", App.tr("Radius"), 1, 4000, 1], ["z", App.tr("Height"), -1000, 2000, 1]].concat(tools.light?.type === "tube" ? [["length", App.tr("Length"), 1, 4000, 1]] : []).concat(tools.light?.type === "spot" ? [["inner", App.tr("Bright middle"), 0, 180, 0.5], ["outer", App.tr("Cone"), 0, 180, 0.5]] : [])
        onEdited: (key, value) => {
            if (key === "inner" || key === "outer")
                tools.page.change(tools.node.pointer + "/cone", key === "inner" ? [Math.min(value, tools.light.cone[1]), tools.light.cone[1]] : [tools.light.cone[0], Math.max(value, tools.light.cone[0])], true);
            else
                tools.page.change(tools.node.pointer + "/" + key, value, true);
        }
    }

    QQC2.Label {
        text: App.tr("Light against the distance")
    }

    SceneCurvePlot {
        Layout.fillWidth: true
        curve: tools.falloff?.curve ?? null
        marks: tools.falloff?.marks ?? []
    }

    Flow {
        Layout.fillWidth: true

        QQC2.CheckBox {
            text: App.tr("Show the part it redraws when it moves")
            checked: tools.page.lightBoxes
            onToggled: tools.page.lightBoxes = checked
        }

        QQC2.CheckBox {
            text: App.tr("Light the preview with it alone")
            checked: tools.light?.solo === true
            onToggled: tools.page.run("light", {
                "solo": {
                    "layer": checked ? tools.lamp : null
                }
            })
        }

        QQC2.CheckBox {
            text: App.tr("Leave it out of the preview")
            checked: tools.light?.muted === true
            onToggled: tools.page.run("light", {
                "mute": {
                    "layer": tools.lamp,
                    "muted": checked
                }
            })
        }
    }
}
