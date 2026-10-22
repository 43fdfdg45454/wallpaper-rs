import QtQuick
import org.kde.plasma.workspace.dbus as DBus

QtObject {
    id: daemon

    property var current: ({})
    property var schema: ({})
    property var playlists: []
    property var profiles: []
    property string pollError: ""
    property string actionError: ""
    readonly property string lastError: actionError || pollError
    readonly property var fields: ({
            "pause.when_covered": i18n("Pause when a window covers the screen"),
            "pause.on_battery": i18n("Pause on battery"),
            "pause.when_locked": i18n("Pause while the screen is locked"),
            "playback.max_fps": i18n("Frames per second"),
            "transition.kind": i18n("Transition"),
            "transition.duration_ms": i18n("Transition length (ms)")
        })
    readonly property var values: ({
            "fill": i18nc("@item:inlistbox wallpaper fit mode", "Fill"),
            "fit": i18nc("@item:inlistbox wallpaper fit mode", "Fit"),
            "stretch": i18nc("@item:inlistbox wallpaper fit mode", "Stretch"),
            "center": i18nc("@item:inlistbox wallpaper fit mode", "Center"),
            "tile": i18nc("@item:inlistbox wallpaper fit mode", "Tile"),
            "span": i18nc("@item:inlistbox wallpaper fit mode", "Span"),
            "fade": i18nc("@item:inlistbox wallpaper transition", "Fade"),
            "none": i18nc("@item:inlistbox wallpaper transition", "None")
        })
    readonly property var signatures: ({
            "Daemon1.SetWallpaper": "(sss)",
            "Daemon1.SetBackground": "(ss)",
            "Daemon1.SetOption": "(ss)",
            "Daemon1.SetPaused": "(b)",
            "Daemon1.SetVolume": "(d)",
            "Daemon1.SetMuted": "(b)",
            "Library1.Thumbnail": "(su)",
            "Library1.GetItem": "(s)",
            "Library1.SetFavorite": "(asb)",
            "Playlists1.Start": "(ss)",
            "Playlists1.Stop": "(s)",
            "Playlists1.Next": "(s)",
            "Playlists1.Previous": "(s)",
            "Profiles1.Activate": "(s)"
        })

    signal updated(string topic, var ids)

    function call(member, args, iface, onReply, onError) {
        const message = {
            "service": "io.github.wallpaper_rs.Daemon",
            "path": "/io/github/wallpaper_rs/Daemon",
            "iface": "io.github.wallpaper_rs." + iface,
            "member": member,
            "signature": signatures[iface + "." + member] ?? "()",
            "arguments": args
        };
        DBus.SessionBus.asyncCall(message, onReply, reply => onError(reply?.error?.message || i18n("The wallpaper-rs daemon is not running")));
    }

    function query(member, args, iface, onReply, onError) {
        call(member, args, iface ?? "Daemon1", reply => {
            const text = String(reply.value ?? "");
            onReply(text.startsWith("{") ? JSON.parse(text) : reply.values.map(String));
        }, onError ?? (() => {}));
    }

    function act(member, args, iface) {
        actionError = "";
        call(member, args, iface ?? "Daemon1", () => {}, error => daemon.actionError = error);
    }

    function refresh(topic) {
        if (!topic || topic === "state")
            query("GetState", [], "Daemon1", reply => {
                if (!(reply.serial < daemon.current.serial))
                    daemon.current = reply;
                daemon.pollError = "";
            }, error => daemon.pollError = error);
        if (!topic)
            query("GetSchema", [], "Daemon1", reply => daemon.schema = reply);
        if (!topic || topic === "playlists")
            query("List", [], "Playlists1", reply => daemon.playlists = reply.playlists ?? []);
        if (!topic || topic === "profiles")
            query("List", [], "Profiles1", reply => daemon.profiles = reply.profiles ?? []);
    }

    function openApp() {
        const message = {
            "service": "io.github.wallpaper_rs",
            "path": "/io/github/wallpaper_rs",
            "iface": "org.freedesktop.Application",
            "member": "Activate",
            "signature": "(a{sv})",
            "arguments": [{}]
        };
        DBus.SessionBus.asyncCall(message, () => {}, reply => daemon.actionError = reply?.error?.message || i18n("wallpaper-rs could not be opened"));
    }

    function tr(text, first, second, third) {
        return i18n(text, first, second, third);
    }

    function field(key) {
        return fields[key] ?? key;
    }

    function value(text) {
        return values[text] ?? String(text ?? "");
    }

    function url(path) {
        return path ? "file://" + path.split("/").map(encodeURIComponent).join("/") : "";
    }

    function path(url) {
        return decodeURIComponent(String(url).replace("file://", ""));
    }
}
