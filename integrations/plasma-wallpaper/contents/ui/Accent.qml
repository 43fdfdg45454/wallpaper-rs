import QtQuick
import QtQuick.Window
import org.kde.plasma.workspace.dbus as DBus

Item {
    id: accent

    required property Item wallpaper
    property string color: ""

    function refresh() {
        DBus.SessionBus.asyncCall({
            "service": "io.github.wallpaper_rs.Daemon",
            "path": "/io/github/wallpaper_rs/Daemon",
            "iface": "io.github.wallpaper_rs.Daemon1",
            "member": "GetState",
            "signature": "()",
            "arguments": []
        }, reply => {
            const text = String(reply.value ?? "");
            const shown = text.startsWith("{") ? JSON.parse(text).outputs?.[Screen.name]?.accent : "";
            accent.color = /^#[0-9a-f]{6}$/.test(shown ?? "") ? shown : "";
        }, () => accent.color = "");
    }

    onColorChanged: {
        if ("accentColor" in wallpaper)
            wallpaper.accentColor = color === "" ? "transparent" : color;
    }
    Component.onCompleted: refresh()

    Connections {
        target: Screen

        function onNameChanged() {
            accent.refresh();
        }
    }

    DBus.SignalWatcher {
        service: "io.github.wallpaper_rs.Daemon"
        path: "/io/github/wallpaper_rs/Daemon"
        iface: "io.github.wallpaper_rs.Daemon1"

        function dbusChanged(topic, ids, serial) {
            if (String(topic) === "state")
                accent.refresh();
        }
    }
}
