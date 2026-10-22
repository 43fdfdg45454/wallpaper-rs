import QtQuick
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.workspace.dbus as DBus

PlasmoidItem {
    id: root

    readonly property Daemon daemon: Daemon {}
    readonly property bool paused: daemon.current.paused === true

    Plasmoid.icon: paused ? "media-playback-pause" : "preferences-desktop-wallpaper"
    Plasmoid.status: PlasmaCore.Types.ActiveStatus
    toolTipMainText: "wallpaper-rs"
    toolTipSubText: daemon.lastError || (paused ? i18n("Paused") : i18n("Playing"))
    fullRepresentation: Panel {
        daemon: root.daemon
    }
    Component.onCompleted: daemon.refresh()
    onExpandedChanged: {
        if (expanded)
            daemon.refresh();
    }

    DBus.SignalWatcher {
        service: "io.github.wallpaper_rs.Daemon"
        path: "/io/github/wallpaper_rs/Daemon"
        iface: "org.freedesktop.DBus.Properties"

        function dbusPropertiesChanged() {
            root.daemon.refresh("state");
        }
    }

    DBus.SignalWatcher {
        service: "io.github.wallpaper_rs.Daemon"
        path: "/io/github/wallpaper_rs/Daemon"
        iface: "io.github.wallpaper_rs.Daemon1"

        function dbusChanged(topic, ids) {
            root.daemon.refresh(String(topic));
            root.daemon.updated(String(topic), Array.from(ids));
        }

        function dbusJobProgress() {
        }

        function dbusJobFinished() {
        }

        function dbusStreamChanged() {
        }
    }

    DBus.SignalWatcher {
        service: "org.freedesktop.DBus"
        path: "/org/freedesktop/DBus"
        iface: "org.freedesktop.DBus"

        function dbusNameOwnerChanged(name, before, after) {
            if (String(name) !== "io.github.wallpaper_rs.Daemon" || String(after) === "")
                return;
            root.daemon.refresh();
            root.daemon.updated("library", []);
        }
    }

    Timer {
        interval: 8000
        running: root.daemon.actionError !== ""
        onTriggered: root.daemon.actionError = ""
    }
}
