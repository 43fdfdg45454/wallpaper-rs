import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: shadows

    required property var page
    required property var lights
    property var info: null
    readonly property var sizes: [0, 256, 512, 1024, 2048, 4096]
    readonly property bool shown: (JSON.parse(page.editor.value("/scene/ext/editor/debug3d") || "null") ?? "") === "shadows"

    function load() {
        info = JSON.parse(page.editor.rig("", JSON.stringify({
            "shadows3d": {
                "time": page.editor.time
            }
        })) || "null");
    }

    function cast(id, args) {
        page.run("scene3d", {
            "shadows": Object.assign({
                "layer": id
            }, args)
        }, true);
    }

    function entry(id) {
        return (info?.lights ?? []).find(l => l.id === id) ?? null;
    }

    visible: lights.some(l => ["point", "spot", "directional"].includes(l.type))
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            shadows.load();
        }

        target: shadows.page.editor
    }

    Kirigami.Heading {
        level: 5
        text: App.tr("Shadows")
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("At most %1 lights cast shadows; tiers draw %2.", shadows.info?.most ?? 2, (shadows.info?.caps ?? []).map(c => c.tier.toUpperCase() + " " + c.lights).join(", "))
    }

    Repeater {
        model: shadows.lights.filter(l => ["point", "spot", "directional"].includes(l.type))

        delegate: ColumnLayout {
            id: light

            required property var modelData
            readonly property var found: shadows.entry(modelData.id)

            Layout.fillWidth: true

            RowLayout {
                QQC2.CheckBox {
                    text: App.tr("%1 casts shadows", light.modelData.id)
                    checked: light.found !== null
                    onToggled: shadows.cast(light.modelData.id, {
                        "on": checked
                    })
                }

                QQC2.ComboBox {
                    enabled: light.found !== null
                    model: shadows.sizes.map(v => v === 0 ? App.tr("Automatic") : App.tr("%1 px", v))
                    currentIndex: Math.max(0, shadows.sizes.indexOf(light.found?.resolution ?? 0))
                    onActivated: index => shadows.cast(light.modelData.id, index === 0 ? {
                            "automatic": true
                        } : {
                            "resolution": shadows.sizes[index]
                        })
                }
            }

            SceneSliders {
                Layout.fillWidth: true
                visible: light.found !== null
                value: ({
                        "bias": light.found?.bias ?? 0.2
                    })
                rows: [["bias", App.tr("Bias"), 0, 1, 0.01]]
                onEdited: (key, value) => shadows.cast(light.modelData.id, {
                        "bias": value
                    })
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: light.found !== null
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
                text: (light.found?.tiers ?? []).map(t => t.tier.toUpperCase() + ": " + (t.shown && t.side > 0 ? App.tr("%1 px, %2 px on phones, %3 px at Q0", t.side, t.phone, t.low) : App.tr("none"))).join(" · ")
            }
        }
    }

    QQC2.CheckBox {
        text: App.tr("Show where shadows fall and the frusta of the maps")
        checked: shadows.shown
        onToggled: shadows.page.run("scene3d", {
            "debug": checked ? {
                "show": "shadows"
            } : {}
        }, true)
    }
}
