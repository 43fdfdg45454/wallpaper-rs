import QtQuick
import QtQuick.Window
import org.kde.plasma.plasmoid
import org.kde.pipewire as PipeWire
import org.kde.plasma.workspace.dbus as DBus

WallpaperItem {
    id: root

    readonly property string output: Screen.name || "default"
    readonly property size pixels: Qt.size(Math.max(1, Math.round(width * Screen.devicePixelRatio)), Math.max(1, Math.round(height * Screen.devicePixelRatio)))
    readonly property bool hasObjectSerial: source.objectSerial !== undefined
    property string lastError: ""
    property bool requesting: false

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
        }, reply => {
            root.requesting = false;
            root.lastError = reply?.error?.message ?? i18n("the daemon (wprsd) is not running");
        });
    }

    function number(value) {
        return Number(value?.value ?? value);
    }

    function requestStream() {
        if (requesting || width <= 0 || height <= 0)
            return;
        requesting = true;
        inFlightReset.restart();
        call("RequestStream", [output, pixels.width, pixels.height], reply => {
            requesting = false;
            const values = reply.values?.length >= 2 ? reply.values : [reply.value, 0];
            show(number(values[0]), number(values[1]));
            call("SetOutputCovered", [output, coverage.covered]);
        });
    }

    function show(node, serial) {
        try {
            if (serial > 0 && hasObjectSerial)
                source.objectSerial = serial;
            else if (node > 0 && source.nodeId !== node)
                source.nodeId = node;
        } catch (e) {
            lastError = String(e);
        }
    }

    onPixelsChanged: resize.restart()
    onOutputChanged: resize.restart()
    Component.onCompleted: resize.restart()
    Component.onDestruction: call("ReleaseStream", [output])

    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    PipeWire.PipeWireSourceItem {
        id: source

        anchors.fill: parent
    }

    Text {
        anchors.centerIn: parent
        width: parent.width * 0.8
        visible: !source.ready && root.lastError !== ""
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        color: "#a0a0a0"
        text: "wallpaper-rs: " + root.lastError
    }

    Coverage {
        id: coverage

        onCoveredChanged: root.call("SetOutputCovered", [root.output, covered])
    }

    Timer {
        id: inFlightReset

        interval: 5000
        onTriggered: root.requesting = false
    }

    Timer {
        id: resize

        interval: 250
        onTriggered: root.requestStream()
    }

    Timer {
        interval: source.ready ? 15000 : 3000
        running: true
        repeat: true
        onTriggered: root.requestStream()
    }
}
