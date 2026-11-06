import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: panel

    required property var page
    property string pointer: ""
    readonly property var line: pointer ? (page.view, JSON.parse(page.editor.value(pointer) || "null")) : null
    readonly property bool counted: line?.frames !== undefined
    readonly property real length: !line ? 0 : counted ? line.frames / Math.max(1, line.fps ?? 1) : (line.duration ?? 0)
    readonly property var cause: pointer ? (page.view, JSON.parse(page.editor.inspect(pointer) || "[]").find(f => f.key === "trigger") ?? {}) : ({})
    readonly property var triggers: cause.options ?? []
    readonly property var loops: ["once", "repeat", "pingpong"]

    function put(fields, gesture) {
        page.run("timeline", Object.assign({
                    "id": line.id
                }, fields), gesture === true);
    }

    function number(text) {
        const v = Number(String(text).replace(",", "."));
        return isFinite(v) ? v : null;
    }

    title: line ? App.tr("Timeline %1", line.id) : ""
    standardButtons: Kirigami.Dialog.Close
    padding: Kirigami.Units.largeSpacing

    Kirigami.FormLayout {
        enabled: panel.line !== null

        QQC2.ComboBox {
            Kirigami.FormData.label: App.tr("Length in:")
            model: [App.tr("Seconds"), App.tr("Frames")]
            currentIndex: panel.counted ? 1 : 0
            onActivated: index => panel.put(index === 1 ? {
                        "frames": Math.max(1, Math.round(panel.length * 30)),
                        "fps": 30
                    } : {
                        "duration": Math.max(0.1, panel.length)
                    })
        }

        QQC2.TextField {
            Kirigami.FormData.label: App.tr("Seconds:")
            visible: !panel.counted
            text: panel.line?.duration ?? ""
            validator: DoubleValidator {
                bottom: 0
            }
            onEditingFinished: {
                const v = panel.number(text);
                if (v !== null && v !== panel.line.duration)
                    panel.put({
                            "duration": v
                        });
            }
        }

        RowLayout {
            Kirigami.FormData.label: App.tr("Frames:")
            visible: panel.counted

            QQC2.SpinBox {
                from: 1
                to: 1000000
                editable: true
                value: panel.line?.frames ?? 1
                onValueModified: panel.put({
                        "frames": value
                    })
            }

            QQC2.Label {
                text: App.tr("at")
            }

            QQC2.SpinBox {
                from: 1
                to: 240
                editable: true
                value: panel.line?.fps ?? 30
                textFromValue: (v, locale) => App.tr("%1 fps", v)
                valueFromText: (text, locale) => parseInt(text) || 1
                onValueModified: panel.put({
                        "fps": value
                    })
            }
        }

        QQC2.ComboBox {
            Kirigami.FormData.label: App.tr("Plays:")
            model: [App.tr("Once"), App.tr("Repeating"), App.tr("Back and forth")]
            currentIndex: Math.max(0, panel.loops.indexOf(panel.line?.loop ?? "once"))
            onActivated: index => panel.put({
                        "loop": panel.loops[index]
                    })
        }

        QQC2.CheckBox {
            text: App.tr("Blend the end into the start")
            enabled: (panel.line?.loop ?? "once") !== "once"
            checked: panel.line?.wrap === true
            onToggled: panel.put({
                    "wrap": checked
                })
        }

        RowLayout {
            Kirigami.FormData.label: App.tr("Speed:")

            SceneSlider {
                editor: panel.page.editor
                from: -4
                to: 4
                stepSize: 0.05
                value: panel.line?.speed ?? 1
                onMoved: panel.put({
                        "speed": Math.round(value * 100) / 100
                    }, true)
            }

            QQC2.TextField {
                implicitWidth: Kirigami.Units.gridUnit * 4
                text: panel.line?.speed ?? 1
                onEditingFinished: {
                    const v = panel.number(text);
                    if (v !== null)
                        panel.put({
                                "speed": v
                            });
                }
            }
        }

        QQC2.TextField {
            Kirigami.FormData.label: App.tr("Delay in seconds:")
            text: panel.line?.delay ?? 0
            validator: DoubleValidator {
                bottom: 0
            }
            onEditingFinished: {
                const v = panel.number(text);
                if (v !== null && v !== (panel.line.delay ?? 0))
                    panel.put({
                            "delay": v
                        });
            }
        }

        QQC2.CheckBox {
            text: App.tr("Start on its own")
            checked: panel.line?.autoplay ?? true
            onToggled: panel.put({
                    "autoplay": checked
                })
        }

        QQC2.ComboBox {
            Kirigami.FormData.label: App.tr("Started by:")
            model: [App.tr("Nothing")].concat(panel.triggers.map((t, i) => (panel.cause.names ?? [])[i] || SceneWords.label(t)))
            currentIndex: panel.line?.trigger ? panel.triggers.indexOf(panel.line.trigger) + 1 : 0
            onActivated: index => panel.put({
                        "trigger": index === 0 ? "" : panel.triggers[index - 1]
                    })
        }

        SceneTimelineEvents {
            Kirigami.FormData.label: App.tr("Events:")
            Layout.fillWidth: true
            Layout.preferredWidth: Kirigami.Units.gridUnit * 22
            panel: panel
        }
    }
}
