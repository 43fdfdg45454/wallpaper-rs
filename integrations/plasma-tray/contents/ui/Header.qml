import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents3

GridLayout {
    id: header

    required property var daemon
    readonly property var current: daemon.current
    readonly property var profiles: daemon.profiles
    readonly property bool paused: current.paused === true
    readonly property bool muted: current.muted === true

    enabled: daemon.pollError === ""
    columns: 2
    columnSpacing: Kirigami.Units.smallSpacing
    rowSpacing: Kirigami.Units.smallSpacing

    PlasmaComponents3.ToolButton {
        Layout.preferredWidth: Math.max(implicitWidth, twin.implicitWidth)
        icon.name: header.paused ? "media-playback-start" : "media-playback-pause"
        text: header.paused ? i18n("Resume") : i18n("Pause")
        onClicked: header.daemon.act("SetPaused", [!header.paused])
    }

    PlasmaComponents3.ToolButton {
        id: twin

        visible: false
        icon.name: "media-playback-start"
        text: header.paused ? i18n("Pause") : i18n("Resume")
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        PlasmaComponents3.ToolButton {
            icon.name: header.muted ? "audio-volume-muted" : "audio-volume-high"
            text: header.muted ? i18n("Unmute") : i18n("Mute")
            display: PlasmaComponents3.AbstractButton.IconOnly
            PlasmaComponents3.ToolTip.text: text
            PlasmaComponents3.ToolTip.visible: hovered
            onClicked: header.daemon.act("SetMuted", [!header.muted])
        }

        PlasmaComponents3.Slider {
            id: volume

            Layout.fillWidth: true
            from: 0
            to: 1
            value: header.current.volume ?? 0
            onMoved: debounce.restart()
        }

        PlasmaComponents3.BusyIndicator {
            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
            visible: (header.current.jobs ?? 0) > 0
            running: visible && Window.window?.visible === true
        }
    }

    Repeater {
        model: [["playback.scene_sound", i18n("Let scenes play their sounds"), true], ["playback.scene_capture", i18n("Let scenes react to the sound that plays"), true], ["playback.flash_guard", i18n("Slow down or stop wallpapers that flash more than three times a second"), false]]

        delegate: PlasmaComponents3.Switch {
            required property var modelData

            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: modelData[1]
            checked: ((header.current.options ?? {})[modelData[0]] ?? modelData[2]) === true
            onToggled: {
                header.daemon.act("SetOption", [modelData[0], checked ? "true" : "false"]);
                checked = Qt.binding(() => ((header.current.options ?? {})[modelData[0]] ?? modelData[2]) === true);
            }
        }
    }

    PlasmaComponents3.ComboBox {
        id: chooser

        Layout.columnSpan: 2
        Layout.fillWidth: true
        model: [i18n("No profile")].concat(header.profiles.map(profile => profile.name))
        currentIndex: header.profiles.findIndex(profile => profile.uid === header.current.profile) + 1
        displayText: currentIndex > 0 ? i18n("Profile: %1", currentText) : currentText
        onActivated: index => header.daemon.act("Activate", [index > 0 ? header.profiles[index - 1].uid : ""], "Profiles1")

        contentItem: PlasmaComponents3.Label {
            text: chooser.displayText
            textFormat: Text.PlainText
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
    }

    Timer {
        id: debounce

        interval: 200
        onTriggered: {
            header.daemon.act("SetVolume", [volume.value]);
            if (header.muted)
                header.daemon.act("SetMuted", [false]);
        }
    }
}
