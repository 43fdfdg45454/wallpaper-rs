import QtQuick
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.workspace.dbus as DBus

PlasmoidItem {
    id: root

    property var daemonState: ({})
    property string pollError: ""
    property string actionError: ""
    readonly property string lastError: actionError || pollError
    readonly property bool paused: daemonState.paused === true

    function call(member, args, onReply, onError) {
        const signatures = {
            "GetState": "()",
            "Reload": "()",
            "SetWallpaper": "(sss)",
            "SetBackground": "(ss)",
            "SetOption": "(ss)",
            "SetPaused": "(b)",
            "SetVolume": "(d)",
            "SetMuted": "(b)"
        };
        const message = {
            "service": "io.github.wallpaper_rs.Daemon",
            "path": "/io/github/wallpaper_rs/Daemon",
            "iface": "io.github.wallpaper_rs.Daemon1",
            "member": member,
            "signature": signatures[member],
            "arguments": args
        };
        DBus.SessionBus.asyncCall(message, onReply, reply => onError(reply?.error?.message || i18n("The wallpaper-rs daemon is not running")));
    }

    function refresh() {
        call("GetState", [], reply => {
            try {
                root.daemonState = JSON.parse(String(reply.value));
                root.pollError = "";
            } catch (e) {
                root.pollError = String(e);
            }
        }, error => root.pollError = error);
    }

    function openApp() {
        const message = {
            "service": "io.github.wallpaper_rs",
            "path": "/io/github/wallpaper_rs",
            "iface": "org.freedesktop.Application",
            "member": "Activate",
            "signature": "(a{sv})",
            "arguments": [
                {}
            ]
        };
        DBus.SessionBus.asyncCall(message, () => {}, reply => {
            root.actionError = reply?.error?.message || i18n("wallpaper-rs could not be opened");
            errorTimeout.restart();
        });
    }

    function act(member, args) {
        actionError = "";
        call(member, args, () => root.refresh(), error => {
            root.actionError = error;
            errorTimeout.restart();
        });
    }

    Plasmoid.icon: paused ? "media-playback-pause" : "preferences-desktop-wallpaper"
    Plasmoid.status: PlasmaCore.Types.ActiveStatus
    toolTipMainText: "wallpaper-rs"
    toolTipSubText: lastError || (paused ? i18n("Paused") : i18n("Playing"))
    fullRepresentation: Panel {
        daemon: root
    }
    Component.onCompleted: refresh()

    DBus.SignalWatcher {
        service: "io.github.wallpaper_rs.Daemon"
        path: "/io/github/wallpaper_rs/Daemon"
        iface: "org.freedesktop.DBus.Properties"

        function dbusPropertiesChanged() {
            root.refresh();
        }
    }

    Timer {
        interval: 2000
        running: root.expanded
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

    Timer {
        id: errorTimeout

        interval: 8000
        onTriggered: root.actionError = ""
    }
}
