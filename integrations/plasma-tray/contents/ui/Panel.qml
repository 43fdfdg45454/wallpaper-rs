import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.plasma.extras as PlasmaExtras

ColumnLayout {
    id: panel

    required property var daemon
    readonly property var current: daemon.current
    readonly property var profiles: daemon.profiles

    Layout.minimumWidth: Kirigami.Units.gridUnit * 26
    spacing: Kirigami.Units.smallSpacing

    RowLayout {
        PlasmaExtras.Heading {
            Layout.fillWidth: true
            level: 3
            text: "wallpaper-rs"
        }

        QQC2.BusyIndicator {
            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
            visible: (panel.current.jobs ?? 0) > 0
            running: visible && Window.window?.visible === true
        }

        QQC2.ComboBox {
            model: [i18n("No profile")].concat(panel.profiles.map(profile => profile.name))
            displayText: i18n("Profile: %1", currentText)
            currentIndex: panel.profiles.findIndex(profile => profile.uid === panel.current.profile) + 1
            onActivated: index => panel.daemon.act("Activate", [index > 0 ? panel.profiles[index - 1].uid : ""], "Profiles1")
        }
    }

    Consent {
        Layout.fillWidth: true
        daemon: panel.daemon
        inputs: panel.daemon.inputs
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
        model: Object.keys(panel.current.outputs ?? {}).concat([""])

        delegate: OutputRow {
            required property string modelData

            Layout.fillWidth: true
            daemon: panel.daemon
            name: modelData
            info: (panel.current.outputs ?? {})[modelData] ?? {}
            playlists: panel.daemon.playlists
        }
    }

    Kirigami.FormLayout {
        Settings {
            daemon: panel.daemon
            model: (panel.daemon.schema.options ?? []).filter(field => field.key in panel.daemon.fields)
            values: panel.current.options ?? {}
            onEdited: (key, value) => panel.daemon.act("SetOption", [key, value])
        }
    }

    RowLayout {
        QQC2.Button {
            icon.name: "view-refresh"
            text: i18n("Reload configuration")
            onClicked: panel.daemon.act("Reload", [])
        }

        QQC2.Button {
            icon.name: "preferences-desktop-wallpaper"
            text: i18n("Open wallpaper-rs")
            onClicked: panel.daemon.openApp()
        }
    }
}
