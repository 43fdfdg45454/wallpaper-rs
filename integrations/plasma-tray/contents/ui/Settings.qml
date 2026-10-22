import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.FormLayout {
    id: settings

    required property var daemon
    required property var options
    readonly property var labels: ({
            "pause.when_covered": i18n("Pause when a window covers the screen"),
            "pause.on_battery": i18n("Pause on battery"),
            "pause.when_locked": i18n("Pause while the screen is locked"),
            "playback.max_fps": i18n("Frames per second:"),
            "transition.kind": i18n("Transition:"),
            "transition.duration_ms": i18n("Transition length (ms):")
        })
    readonly property var kinds: [
        {
            "value": "fade",
            "text": i18nc("@item:inlistbox wallpaper transition", "Fade")
        },
        {
            "value": "none",
            "text": i18nc("@item:inlistbox wallpaper transition", "None")
        }
    ]

    function set(key, value) {
        daemon.act("SetOption", [key, String(value)]);
    }

    Repeater {
        model: Object.keys(settings.labels)

        delegate: Loader {
            required property string modelData
            readonly property var value: settings.options[modelData]

            Kirigami.FormData.label: typeof value === "boolean" ? "" : settings.labels[modelData]
            sourceComponent: modelData === "transition.kind" ? kind : typeof value === "boolean" ? toggle : number
        }
    }

    Component {
        id: toggle

        QQC2.CheckBox {
            text: settings.labels[parent.modelData]
            checked: parent.value === true
            onToggled: settings.set(parent.modelData, checked)
        }
    }

    Component {
        id: number

        QQC2.SpinBox {
            from: parent.modelData === "playback.max_fps" ? 1 : 0
            to: parent.modelData === "playback.max_fps" ? 240 : 10000
            stepSize: parent.modelData === "playback.max_fps" ? 1 : 100
            value: parent.value ?? 0
            onValueModified: settings.set(parent.modelData, value)
        }
    }

    Component {
        id: kind

        QQC2.ComboBox {
            model: settings.kinds
            textRole: "text"
            valueRole: "value"
            currentIndex: Math.max(0, settings.kinds.findIndex(option => option.value === parent.value))
            onActivated: settings.set(parent.modelData, currentValue)
        }
    }
}
