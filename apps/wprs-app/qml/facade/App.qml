pragma Singleton
import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    id: app

    readonly property string stale: "io.github.wallpaper_rs.Error.Stale"
    readonly property string conflict: "io.github.wallpaper_rs.Error.Conflict"
    readonly property string invalidArgs: "org.freedesktop.DBus.Error.InvalidArgs"
    readonly property Backend backend: Backend {
        onReplied: (tag, json) => app.settle(tag, 0, JSON.parse(json))
        onFailed: (tag, name, message) => app.settle(tag, 1, {
                "name": name,
                "message": message
            })
        onChanged: (topic, ids) => app.updated(topic, JSON.parse(ids))
    }
    readonly property var state: JSON.parse(backend.state || "{}")
    readonly property var schema: JSON.parse(backend.schema || "{}")
    readonly property var jobs: JSON.parse(backend.jobs || "[]")
    readonly property var outputs: Object.keys(state.outputs ?? {})
    readonly property bool checking: backend.checking
    property int serial: 0
    property var pending: ({})

    signal navigate(int page, string query)
    signal edit(var item)
    signal updated(string topic, var ids)

    function settle(tag, slot, value) {
        const handler = (pending[tag] ?? [])[slot];
        delete pending[tag];
        if (handler)
            handler(value);
    }

    function remember(onReply, onError) {
        const tag = String(++serial);
        pending[tag] = [onReply, onError];
        return tag;
    }

    function query(member, args, iface, onReply, onError) {
        backend.call(remember(onReply, onError), iface ?? "Daemon1", member, JSON.stringify(args));
    }

    function act(member, args, iface, onDone, onError) {
        query(member, args, iface, onDone, onError);
    }

    function addProvider(id, kind, options, secret, onDone, onError) {
        backend.addProvider(remember(onDone, onError), id, kind, JSON.stringify(options), secret);
    }

    function setSecret(id, secret, onDone, onError) {
        backend.setSecret(remember(onDone, onError), id, secret);
    }

    function tr(text, first, second, third) {
        return [first, second, third].filter(arg => arg !== undefined).reduce((out, arg) => out.arg(arg), backend.tr(text));
    }

    function field(key) {
        return Labels.field(key);
    }

    function value(text) {
        return Labels.value(text);
    }

    function url(path) {
        return path ? backend.url(path) : "";
    }

    function path(url) {
        return backend.path(String(url));
    }

    function time(seconds) {
        return seconds ? new Date(seconds * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat) : "";
    }

    function bytes(count) {
        return count >= 1048576 ? tr("%1 MB").arg(Math.round(count / 1048576)) : tr("%1 KB").arg(Math.ceil(count / 1024));
    }
}
