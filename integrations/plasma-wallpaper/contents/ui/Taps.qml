import QtQuick
import QtQuick.Window
import org.kde.plasma.workspace.dbus as DBus

Item {
    id: taps

    readonly property string output: Screen.name
    property bool taking: false

    function message(member, signature, args) {
        return {
            "service": "io.github.wallpaper_rs.Daemon",
            "path": "/io/github/wallpaper_rs/Daemon",
            "iface": "io.github.wallpaper_rs.Inputs1",
            "member": member,
            "signature": signature,
            "arguments": args
        };
    }

    function refresh() {
        DBus.SessionBus.asyncCall(message("GetInputs", "()", []), reply => {
            const text = String(reply.value ?? "");
            taps.taking = text.startsWith("{") && (JSON.parse(text).clicking ?? []).includes(taps.output);
        }, () => taps.taking = false);
    }

    function send(point, buttons) {
        if (taps.taking && width > 0 && height > 0)
            DBus.SessionBus.asyncCall(message("Touch", "(sddu)", [output, point.x / width, point.y / height, buttons]));
    }

    onOutputChanged: refresh()
    Component.onCompleted: refresh()

    PointHandler {
        id: press

        enabled: taps.taking
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onActiveChanged: taps.send(press.point.position, press.active ? press.point.pressedButtons : 0)
    }

    DBus.SignalWatcher {
        service: "io.github.wallpaper_rs.Daemon"
        path: "/io/github/wallpaper_rs/Daemon"
        iface: "io.github.wallpaper_rs.Daemon1"

        function dbusChanged(topic, ids, serial) {
            if (["inputs", "state"].includes(String(topic)))
                taps.refresh();
        }
    }
}
