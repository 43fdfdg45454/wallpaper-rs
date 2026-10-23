import QtQuick
import org.kde.plasma.workspace.dbus as DBus

QtObject {
    id: daemon

    property var current: ({})
    property var inputs: ({})
    property var profiles: []
    property string pollError: ""
    property string actionError: ""
    property bool hasApp: true
    readonly property string lastError: actionError || pollError
    readonly property var signatures: ({
            "Daemon1.SetWallpaper": "(sss)",
            "Daemon1.SetPaused": "(b)",
            "Daemon1.SetVolume": "(d)",
            "Daemon1.SetMuted": "(b)",
            "Inputs1.Grant": "(ss)",
            "Inputs1.Revoke": "(ss)",
            "Inputs1.GetInputs": "()",
            "Library1.Thumbnail": "(su)",
            "Library1.GetItem": "(s)",
            "Library1.SetFavorite": "(asb)",
            "Playlists1.Next": "(s)",
            "Profiles1.Activate": "(s)"
        })

    signal updated(string topic, var ids)

    function send(message, onReply, onError) {
        DBus.SessionBus.asyncCall(message, onReply, onError);
    }

    function call(member, args, iface, onReply, onError) {
        send({
            "service": "io.github.wallpaper_rs.Daemon",
            "path": "/io/github/wallpaper_rs/Daemon",
            "iface": "io.github.wallpaper_rs." + iface,
            "member": member,
            "signature": signatures[iface + "." + member] ?? "()",
            "arguments": args
        }, onReply, reply => onError(reply?.error?.message || i18n("The wallpaper-rs daemon is not running")));
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
        if (!topic || topic === "profiles")
            query("List", [], "Profiles1", reply => daemon.profiles = reply.profiles ?? []);
        if (!topic || topic === "inputs" || topic === "state")
            query("GetInputs", [], "Inputs1", reply => daemon.inputs = reply);
        if (!topic)
            findApp();
    }

    function bus(member, args, onReply) {
        send({
            "service": "org.freedesktop.DBus",
            "path": "/org/freedesktop/DBus",
            "iface": "org.freedesktop.DBus",
            "member": member,
            "signature": args.length ? "(s)" : "()",
            "arguments": args
        }, onReply, () => {});
    }

    function findApp() {
        const app = "io.github.wallpaper_rs";
        bus("ListActivatableNames", [], reply => {
            const names = Array.from(reply.value ?? []).map(String);
            if (names.includes(app) || !names.includes(app + ".Daemon"))
                daemon.hasApp = true;
            else
                bus("NameHasOwner", [app], reply => daemon.hasApp = reply.value === true);
        });
    }

    function openApp(page) {
        send({
            "service": "io.github.wallpaper_rs",
            "path": "/io/github/wallpaper_rs",
            "iface": "org.freedesktop.Application",
            "member": page ? "ActivateAction" : "Activate",
            "signature": page ? "(sava{sv})" : "(a{sv})",
            "arguments": page ? [page, [], {}] : [{}]
        }, () => {}, reply => daemon.actionError = reply?.error?.message || i18n("wallpaper-rs could not be opened"));
    }

    function tr(text, first, second, third) {
        return i18n(text, first, second, third);
    }

    function url(path) {
        return path ? "file://" + path.split("/").map(encodeURIComponent).join("/") : "";
    }

    function path(url) {
        return decodeURIComponent(String(url).replace("file://", ""));
    }
}
