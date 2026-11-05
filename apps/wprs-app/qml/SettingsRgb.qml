import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: lights

    property var status: ({})
    property string problem: ""
    readonly property var options: App.state.options ?? {}
    readonly property bool on: options["rgb.enabled"] === true
    readonly property string server: String(options["rgb.server"] ?? "127.0.0.1:6742")
    readonly property string covered: String(options["rgb.when_covered"] ?? "pause")

    function load() {
        App.query("GetStatus", [], "Rgb1", reply => lights && (lights.status = reply ?? {}), error => lights && (lights.problem = error.message));
    }

    function ask(member, args) {
        problem = "";
        App.act(member, args, "Rgb1", () => lights && lights.load(), error => lights && lights.failed(error));
    }

    function failed(error) {
        problem = error.message;
        load();
    }

    Component.onCompleted: load()

    Timer {
        interval: 2000
        repeat: true
        running: lights.visible && lights.on
        onTriggered: lights.load()
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.tr("Wallpapers that send colors to keyboards, mice and other lights reach them through an OpenRGB server, and only after you allow it for each package. Turn this on to let them; the address is this computer unless you change it.")
    }

    QQC2.Switch {
        text: App.tr("Send colors to RGB lighting")
        checked: lights.on
        onToggled: App.act("SetOption", ["rgb.enabled", String(checked)], "Daemon1", () => lights && lights.load())
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("OpenRGB server:")
        }

        QQC2.TextField {
            id: address

            Layout.fillWidth: true
            text: lights.server
            placeholderText: "127.0.0.1:6742"
            onEditingFinished: {
                if (text !== lights.server)
                    App.act("SetOption", ["rgb.server", text], "Daemon1", () => lights && lights.load(), error => lights && (lights.problem = error.message));
            }
        }

        QQC2.Button {
            text: App.tr("Connect")
            icon.name: "network-connect"
            enabled: lights.on
            onClicked: lights.ask("Connect", [])
        }
    }

    QQC2.Label {
        text: App.tr("While a window covers the wallpaper:")
    }

    QQC2.RadioButton {
        text: App.tr("Keep following the wallpaper")
        checked: lights.covered === "follow"
        onToggled: if (checked) App.act("SetOption", ["rgb.when_covered", "follow"], "Daemon1", () => lights && lights.load())
    }

    QQC2.RadioButton {
        text: App.tr("Pause and leave the lights as they are")
        checked: lights.covered !== "follow"
        onToggled: if (checked) App.act("SetOption", ["rgb.when_covered", "pause"], "Daemon1", () => lights && lights.load())
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        color: Kirigami.Theme.disabledTextColor
        text: lights.covered === "follow" ? App.tr("The lights keep changing with the scene, which keeps running without drawing the hidden wallpaper.") : App.tr("While the covered wallpaper pauses, nothing is sent and nothing runs for the lights until it shows again.")
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: lights.status.local === false
        type: Kirigami.MessageType.Warning
        text: App.tr("The colors of your wallpaper go over the network to %1. Anyone who can watch that network could see them; use an address on this computer to keep them here.", lights.server)
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        color: lights.problem || lights.status.error ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
        text: lights.problem || lights.status.error || (lights.status.connected ? App.tr("Connected; %1 sends the colors now.", lights.status.source ?? App.tr("nothing")) : App.tr("Not connected."))
    }

    Repeater {
        model: lights.status.devices ?? []

        delegate: RowLayout {
            required property var modelData

            QQC2.Label {
                Layout.fillWidth: true
                textFormat: Text.PlainText
                text: App.tr("%1, with %2 lights", modelData.name, modelData.leds)
            }

            QQC2.Button {
                text: App.tr("Test")
                icon.name: "color-management"
                enabled: !lights.status.testing
                onClicked: lights.ask("Test", [modelData.index])
            }
        }
    }

    QQC2.Button {
        visible: (lights.status.devices ?? []).length > 1
        text: App.tr("Test every device")
        icon.name: "color-management"
        enabled: !lights.status.testing
        onClicked: lights.ask("Test", [-1])
    }
}
