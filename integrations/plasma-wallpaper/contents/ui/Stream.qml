import QtQuick
import QtQuick.Window
import org.kde.pipewire as PipeWire
import org.kde.plasma.workspace.dbus as DBus

Item {
    id: stream

    required property bool covered
    readonly property string output: Screen.name
    readonly property real ratio: Window.window?.devicePixelRatio || Screen.devicePixelRatio
    readonly property size pixels: Qt.size(Math.max(1, Math.round(width * ratio)), Math.max(1, Math.round(height * ratio)))
    readonly property var place: [Screen.virtualX, Screen.virtualY, Screen.width, Screen.height, ratio, Screen.pixelDensity * ratio, [Screen.manufacturer, Screen.model, Screen.serialNumber].filter(part => part).join(" ")]
    readonly property bool wanted: output !== "" && width > 0 && height > 0
    readonly property bool broken: error !== "" || (view.item?.state ?? 0) <= PipeWire.PipeWireSourceItem.Unconnected
    readonly property bool streaming: view.item?.state === PipeWire.PipeWireSourceItem.Streaming
    readonly property bool ready: view.item?.ready ?? false
    readonly property var lifetime: ({
            "alive": true
        })
    property string owned: ""
    property string error: ""
    property bool requesting: false
    property int failures: 0

    function message(member, args) {
        const signatures = {
            "RequestStream": "(suu)",
            "ReleaseStream": "(s)",
            "SetOutputCovered": "(sb)",
            "SetOutputInfo": "(siiuudds)"
        };
        return {
            "service": "io.github.wallpaper_rs.Daemon",
            "path": "/io/github/wallpaper_rs/Daemon",
            "iface": "io.github.wallpaper_rs.Daemon1",
            "member": member,
            "signature": signatures[member],
            "arguments": args
        };
    }

    function send(member, args) {
        DBus.SessionBus.asyncCall(message(member, args));
    }

    function release() {
        if (owned === "")
            return;
        send("SetOutputCovered", [owned, false]);
        send("ReleaseStream", [owned]);
        owned = "";
    }

    function request() {
        if (requesting || !wanted)
            return;
        const target = output, w = pixels.width, h = pixels.height, life = lifetime, bus = DBus.SessionBus;
        const abandon = message("ReleaseStream", [target]);
        requesting = true;
        bus.asyncCall(message("RequestStream", [target, w, h]), reply => life.alive ? stream.received(target, w, h, reply) : bus.asyncCall(abandon), reply => life.alive && stream.failed(reply));
    }

    function received(target, w, h, reply) {
        requesting = false;
        error = "";
        if (target !== output) {
            send("ReleaseStream", [target]);
            resize.restart();
            return;
        }
        owned = target;
        send("SetOutputInfo", [target].concat(place));
        const values = (reply.values ?? []).map(v => Number(v?.value ?? v));
        show(values[0] ?? 0, values[1] ?? 0, broken);
        send("SetOutputCovered", [target, covered]);
        if (w !== pixels.width || h !== pixels.height)
            resize.restart();
    }

    function show(node, serial, reconnect) {
        if (!reconnect && node === view.node && serial === view.serial)
            return;
        view.node = node;
        view.serial = serial;
        view.active = false;
        view.active = true;
    }

    function failed(reply) {
        requesting = false;
        error = reply?.error?.message || i18nd("plasma_wallpaper_io.github.wallpaper_rs", "the daemon (wprsd) is not running");
    }

    onPixelsChanged: resize.restart()
    onPlaceChanged: {
        if (owned !== "")
            send("SetOutputInfo", [owned].concat(place));
    }
    onOutputChanged: {
        release();
        resize.restart();
    }
    onCoveredChanged: {
        if (owned !== "")
            send("SetOutputCovered", [owned, covered]);
    }
    onStreamingChanged: {
        if (streaming)
            failures = 0;
    }
    Component.onCompleted: resize.restart()
    Component.onDestruction: {
        lifetime.alive = false;
        release();
    }

    Loader {
        id: view

        property int node: 0
        property real serial: 0

        anchors.fill: parent
        active: false

        sourceComponent: PipeWire.PipeWireSourceItem {
            id: source

            Component.onCompleted: {
                if (view.serial > 0 && source.objectSerial !== undefined)
                    source.objectSerial = view.serial;
                else
                    source.nodeId = view.node;
            }
        }
    }

    DBus.SignalWatcher {
        service: "io.github.wallpaper_rs.Daemon"
        path: "/io/github/wallpaper_rs/Daemon"
        iface: "io.github.wallpaper_rs.Daemon1"

        function dbusStreamChanged(target, node, serial) {
            if (target === stream.owned)
                stream.show(Number(node), Number(serial), true);
        }
    }

    DBus.DBusServiceWatcher {
        watchedService: "io.github.wallpaper_rs.Daemon"
        onRegisteredChanged: {
            if (!registered)
                return;
            stream.failures = 0;
            view.active = false;
            resize.restart();
        }
    }

    Timer {
        id: resize

        interval: 250
        onTriggered: stream.request()
    }

    Timer {
        interval: [1000, 2000, 5000, 10000][Math.min(stream.failures, 3)]
        running: stream.broken && stream.wanted && !stream.requesting
        repeat: true
        onTriggered: {
            stream.failures++;
            stream.request();
        }
    }
}
