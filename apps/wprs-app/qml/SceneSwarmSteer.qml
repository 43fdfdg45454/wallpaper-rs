import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: steer

    required property var tools
    readonly property var keeps: tools.info?.keeps ?? []
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
            text: App.tr("Distances and flocks")
        }

        QQC2.ComboBox {
            model: [App.tr("Keep a distance"), App.tr("Keep between two points")]
            displayText: App.tr("Add a force")
            onActivated: index => steer.tools.edit("force", {
                        "force": index === 0 ? {
                            "keep_distance": {
                                "target": "p0",
                                "distance": 100,
                                "strength": 4
                            }
                        } : {
                            "keep_between": {
                                "from": "p0",
                                "to": [1200, 540],
                                "strength": 4
                            }
                        }
                    })
        }
    }

    Repeater {
        model: steer.keeps

        delegate: SceneSwarmKeep {
            required property var modelData

            Layout.fillWidth: true
            steer: steer
            keep: modelData
        }
    }

    QQC2.CheckBox {
        text: App.tr("Move in flocks")
        checked: steer.flock !== null
        onToggled: steer.tools.edit("boids", checked ? {} : {
                "off": true
            })
    }

    Repeater {
        model: steer.flock ? [["radius", App.tr("Neighbour radius"), 1, 400], ["separation", App.tr("Separation"), 0, 10], ["alignment", App.tr("Alignment"), 0, 10], ["cohesion", App.tr("Cohesion"), 0, 10], ["max_speed", App.tr("Most speed"), 0, 1000]] : []

        delegate: RowLayout {
            required property var modelData

            QQC2.Label {
                text: modelData[1]
            }

            QQC2.Slider {
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
