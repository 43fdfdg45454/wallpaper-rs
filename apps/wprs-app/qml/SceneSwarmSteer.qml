import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: steer

    required property var tools
    readonly property var keeps: tools.info?.keeps ?? []
    readonly property var pulls: tools.info?.pulls ?? []
    readonly property var fresh: [{
            "keep_distance": {
                "target": "p0",
                "distance": 100,
                "strength": 4
            }
        }, {
            "keep_between": {
                "from": "p0",
                "to": [1200, 540],
                "strength": 4
            }
        }, {
            "attractor": {
                "target": "p0",
                "strength": 100,
                "radius": 300
            }
        }, {
            "vortex": {
                "center": "p0",
                "strength": 100,
                "radius": 300
            }
        }, {
            "orbit": {
                "center": "p0",
                "strength": 60,
                "radius": 300
            }
        }, {
            "slow_near": {
                "target": "p0",
                "inner": 20,
                "outer": 120,
                "factor": 0.8
            }
        }]
    readonly property var flock: tools.info?.boids ?? null
    readonly property var targets: (tools.info?.points ?? []).map(p => p.name).concat(["cursor"])

    function force(index) {
        return JSON.parse(tools.page.editor.value(SceneSwarmKnobs.pointer(tools.system, "/forces/" + index)) || "null");
    }

    function patch(e, change, gesture) {
        const v = force(e.index);
        change(v[e.kind]);
        tools.edit("force", {
                "index": e.index,
                "force": v
            }, gesture);
    }

    RowLayout {
        Kirigami.Heading {
            Layout.fillWidth: true
            level: 4
            text: App.tr("Pulls, distances and flocks")
        }

        QQC2.ComboBox {
            model: [App.tr("Keep a distance"), App.tr("Keep between two points"), App.tr("Pull towards a point"), App.tr("Swirl around a point"), App.tr("Circle around a point"), App.tr("Slow down near a point")]
            displayText: App.tr("Add a force")
            onActivated: index => steer.tools.edit("force", {
                        "force": steer.fresh[index]
                    })
        }
    }

    SceneRepeater {
        id: listed3

        readonly property var host: steer

        rows: steer.keeps

        delegate: SceneSwarmKeep {
            required property int index
            readonly property var modelData: listed3.rows[index]

            Layout.fillWidth: true
            steer: listed3.host
            keep: modelData
        }
    }

    SceneRepeater {
        id: listed2

        readonly property var host: steer

        rows: steer.pulls

        delegate: SceneSwarmPull {
            required property int index
            readonly property var modelData: listed2.rows[index]

            Layout.fillWidth: true
            steer: listed2.host
            pull: modelData
        }
    }

    QQC2.CheckBox {
        text: App.tr("Move in flocks")
        checked: steer.flock !== null
        onToggled: steer.tools.edit("boids", checked ? {} : {
                "off": true
            })
    }

    SceneRepeater {
        id: listed

        rows: steer.flock ? [["radius", App.tr("Neighbour radius"), 1, 400], ["separation", App.tr("Separation"), 0, 10], ["alignment", App.tr("Alignment"), 0, 10], ["cohesion", App.tr("Cohesion"), 0, 10], ["max_speed", App.tr("Most speed"), 0, 1000]] : []

        delegate: RowLayout {
            required property int index
            readonly property var modelData: listed.rows[index]

            QQC2.Label {
                text: modelData[1]
            }

            SceneSlider {
                Layout.fillWidth: true
                from: modelData[2]
                to: modelData[3]
                value: steer.flock.boids[modelData[0]] ?? 0
                onMoved: steer.tools.edit("boids", {
                        "boids": {
                            [modelData[0]]: Math.round(value * 100) / 100
                        }
                    }, true)
            }
        }
    }
}
