import QtQuick
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.workspace.dbus as DBus

PlasmoidItem {
    id: root

    property var daemonState: ({})
    property string lastError: ""
    readonly property bool paused: daemonState.paused === true

    function call(member, args, onReply) {
        const message = {
            "service": "io.github.wallpaper_rs.Daemon",
            "path": "/io/github/wallpaper_rs/Daemon",
            "iface": "io.github.wallpaper_rs.Daemon1",
            "member": member,
            "arguments": args
        };
        DBus.SessionBus.asyncCall(message, reply => {
            root.lastError = "";
            onReply?.(reply);
        }, reply => root.lastError = reply?.error?.message ?? i18n("The wallpaper-rs daemon is not running"));
    }

    function refresh() {
        call("GetState", [], reply => root.daemonState = JSON.parse(reply.value));
    }

    function act(member, args) {
        call(member, args, refresh);
    }

    Plasmoid.icon: paused ? "media-playback-pause" : "preferences-desktop-wallpaper"
    Plasmoid.status: PlasmaCore.Types.ActiveStatus
    toolTipMainText: "wallpaper-rs"
    toolTipSubText: lastError || (paused ? i18n("Paused") : i18n("Playing"))
    fullRepresentation: Panel {
        daemon: root
    }

    Timer {
        interval: root.expanded ? 2000 : 10000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }
}
