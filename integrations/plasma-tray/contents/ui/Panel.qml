import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.plasma.extras as PlasmaExtras

ColumnLayout {
    id: panel

    required property var daemon
    readonly property var current: daemon.daemonState

    Layout.minimumWidth: Kirigami.Units.gridUnit * 24
    spacing: Kirigami.Units.smallSpacing

    PlasmaExtras.Heading {
        level: 3
        text: "wallpaper-rs"
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: panel.daemon.lastError !== ""
        text: panel.daemon.lastError
        color: Kirigami.Theme.negativeTextColor
        wrapMode: Text.Wrap
    }

    RowLayout {
        QQC2.Button {
            icon.name: panel.current.paused ? "media-playback-start" : "media-playback-pause"
            text: panel.current.paused ? i18n("Resume") : i18n("Pause")
            onClicked: panel.daemon.act("SetPaused", [!panel.current.paused])
        }

        QQC2.ToolButton {
            icon.name: panel.current.muted ? "audio-volume-muted" : "audio-volume-high"
            onClicked: panel.daemon.act("SetMuted", [!panel.current.muted])
        }

        QQC2.Slider {
            Layout.fillWidth: true
            from: 0
            to: 1
            value: panel.current.volume ?? 0
            onMoved: {
                volume.level = value;
                volume.restart();
            }
        }
    }

    Timer {
        id: volume

        property real level

        interval: 200
        onTriggered: panel.daemon.act("SetVolume", [level])
    }

    Repeater {
        model: Object.keys(panel.current.outputs ?? {})

        delegate: OutputRow {
            required property string modelData

            daemon: panel.daemon
            name: modelData
            info: panel.current.outputs[modelData]
        }
    }

    OutputRow {
        daemon: panel.daemon
        name: ""
        info: ({
                "fit": Object.values(panel.current.outputs ?? {})[0]?.fit
            })
    }

    Settings {
        daemon: panel.daemon
        options: panel.current.options ?? {}
    }

    QQC2.Button {
        icon.name: "view-refresh"
        text: i18n("Reload configuration")
        onClicked: panel.daemon.act("Reload", [])
    }
}
