import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: emission

    required property var tools
    required property int index
    readonly property var emitter: (tools.info, JSON.parse(tools.page.editor.value(SceneSwarmKnobs.pointer(tools.system, "/emitters/" + index)) || "null")) ?? ({})
    readonly property bool driven: emitter.rate !== undefined && typeof emitter.rate === "object"
    readonly property var cues: ["", "beat", "click"]

    function put(values, gesture) {
        tools.edit("emit", {
                "emitter": index,
                "values": values
            }, gesture);
    }

    function rate(value, replace, gesture) {
        tools.edit("settings", {
                "emitter": index,
                "rate": value,
                "replace": replace
            }, gesture);
    }

    RowLayout {
        QQC2.Label {
            text: emission.driven ? App.tr("Rate follows keyframes or a link") : App.tr("%1 a second", Math.round((emission.emitter.rate ?? 0) * 10) / 10)
        }

        QQC2.Slider {
            Layout.fillWidth: true
            enabled: !emission.driven
            from: 0
            to: Math.max(500, typeof emission.emitter.rate === "number" ? emission.emitter.rate : 0)
            value: typeof emission.emitter.rate === "number" ? emission.emitter.rate : 0
            onPressedChanged: emission.tools.page.editor.hold(pressed)
            onMoved: emission.rate(Math.round(value * 10) / 10, false, true)
        }

        QQC2.Button {
            visible: emission.driven
            text: App.tr("Fixed rate")
            onClicked: emission.rate(10, true)
        }
    }

    SceneSwarmEmitLane {
        Layout.fillWidth: true
        emission: emission
    }

    RowLayout {
        QQC2.CheckBox {
            text: App.tr("Bursts of")
            checked: emission.emitter.burst !== undefined
            onToggled: emission.put({
                    "burst": checked ? {
                        "count": 20,
                        "every": 1
                    } : null
                })
        }

        QQC2.SpinBox {
            enabled: emission.emitter.burst !== undefined
            from: 1
            to: 100000
            editable: true
            value: emission.emitter.burst?.count ?? 20
            onValueModified: emission.put({
                    "burst": Object.assign({}, emission.emitter.burst, {
                            "count": value
                        })
                })
        }

        QQC2.ComboBox {
            enabled: emission.emitter.burst !== undefined
            model: [App.tr("on time"), App.tr("on the beat"), App.tr("on a click")]
            currentIndex: Math.max(0, emission.cues.indexOf(emission.emitter.burst?.on ?? ""))
            onActivated: index => {
                const burst = Object.assign({}, emission.emitter.burst);
                delete burst.on;
                if (index > 0)
                    burst.on = emission.cues[index];
                emission.put({
                        "burst": burst
                    });
            }
        }
    }

    RowLayout {
        QQC2.CheckBox {
            text: App.tr("Follow the pointer")
            checked: emission.emitter.follow === "cursor"
            onToggled: emission.put({
                    "follow": checked ? "cursor" : null
                })
        }

        QQC2.CheckBox {
            text: App.tr("Only while sound plays")
            checked: emission.emitter.audio_gate === true
            onToggled: emission.put({
                    "audio_gate": checked
                })
        }
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Speed taken from the emitter %1", Math.round((emission.emitter.inherit ?? 0) * 100) / 100)
        }

        QQC2.Slider {
            Layout.fillWidth: true
            from: 0
            to: 1
            value: emission.emitter.inherit ?? 0
            onPressedChanged: emission.tools.page.editor.hold(pressed)
            onMoved: emission.put({
                    "inherit": Math.round(value * 100) / 100
                }, true)
        }
    }
}
