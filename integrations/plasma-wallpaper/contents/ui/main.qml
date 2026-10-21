/*
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Shows the frames that the wallpaper-rs daemon renders for this screen.
 * The daemon exports one PipeWire stream per screen; this item asks for a
 * stream of the right size over D-Bus and displays it with KPipeWire.
 */
import QtQuick
import QtQuick.Window
import org.kde.plasma.plasmoid
import org.kde.pipewire as PipeWire
import org.kde.plasma.workspace.dbus as DBus

WallpaperItem {
    id: root

    readonly property string busName: "io.github.wallpaper_rs.Daemon"
    readonly property string objectPath: "/io/github/wallpaper_rs/Daemon"
    readonly property string interfaceName: "io.github.wallpaper_rs.Daemon1"

    // Connector name of this screen, e.g. "DP-1". Used as the output key.
    readonly property string outputName: Screen.name || "default"
    readonly property int pixelWidth: Math.max(1, Math.round(width * Screen.devicePixelRatio))
    readonly property int pixelHeight: Math.max(1, Math.round(height * Screen.devicePixelRatio))

    readonly property string configuredImage: root.configuration.Image || ""
    readonly property string configuredFit: root.configuration.FillMode || "fill"

    property string lastError: ""
    property bool requestInFlight: false

    // Shown until the first frame arrives, and if the daemon is unavailable.
    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    // Must stay visible: KPipeWire only activates the stream of a visible
    // item, so hiding it until the first frame would never show one. It draws
    // nothing until a frame arrives, leaving the black background above.
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

    // Unwraps Plasma D-Bus integer wrappers and plain values alike.
    function toNumber(v) {
        if (v !== null && typeof v === "object" && v.value !== undefined) {
            return Number(v.value);
        }
        return Number(v);
    }

    function call(member, args, onOk) {
        DBus.SessionBus.asyncCall({
            service: root.busName,
            path: root.objectPath,
            iface: root.interfaceName,
            member: member,
            arguments: args
        }, reply => {
            root.lastError = "";
            if (onOk) {
                onOk(reply);
            }
        }, reply => {
            root.requestInFlight = false;
            root.lastError = (reply && reply.error && reply.error.message)
                ? reply.error.message
                : i18n("the daemon (wprsd) is not running");
            console.warn("wallpaper-rs:", member, "failed:", root.lastError);
        });
    }

    function requestStream() {
        if (root.requestInFlight || root.width <= 0 || root.height <= 0) {
            return;
        }
        root.requestInFlight = true;
        root.call("RequestStream", [root.outputName, root.pixelWidth, root.pixelHeight], reply => {
            root.requestInFlight = false;
            try {
                const values = reply.values && reply.values.length >= 2 ? reply.values : [reply.value, 0];
                root.connectStream(root.toNumber(values[0]), root.toNumber(values[1]));
            } catch (e) {
                root.lastError = String(e);
                console.warn("wallpaper-rs: cannot connect to the stream:", e);
            }
        });
        // The error callback does not reset the flag; do it after a timeout.
        inFlightReset.restart();
    }

    // Older KPipeWire releases only have nodeId; objectSerial was added later
    // and assigning a property that does not exist throws.
    readonly property bool hasObjectSerial: source.objectSerial !== undefined

    function connectStream(nodeId, serial) {
        if (serial > 0 && root.hasObjectSerial) {
            if (source.objectSerial !== serial) {
                source.objectSerial = serial;
            }
        } else if (nodeId > 0 && source.nodeId !== nodeId) {
            source.nodeId = nodeId;
        }
    }

    function applyConfiguredImage() {
        if (root.configuredImage === "") {
            return;
        }
        const image = root.configuredImage;
        const path = image.startsWith("file://") ? image.slice(7) : image;
        root.call("SetWallpaper", [decodeURIComponent(path), root.outputName, root.configuredFit], null);
    }

    Timer {
        id: inFlightReset
        interval: 5000
        onTriggered: root.requestInFlight = false
    }

    // Coalesces the initial image, path and fill mode changes into one call.
    Timer {
        id: applyDebounce
        interval: 100
        onTriggered: root.applyConfiguredImage()
    }

    // Coalesces resize and scale changes into one request.
    Timer {
        id: resizeDebounce
        interval: 250
        onTriggered: root.requestStream()
    }

    // Recovers when the daemon starts later or restarts. Reusing an existing
    // stream is cheap: the daemon only answers with the same serial.
    Timer {
        interval: source.ready ? 15000 : 3000
        running: true
        repeat: true
        onTriggered: root.requestStream()
    }

    onPixelWidthChanged: resizeDebounce.restart()
    onPixelHeightChanged: resizeDebounce.restart()
    onOutputNameChanged: resizeDebounce.restart()
    onConfiguredImageChanged: applyDebounce.restart()
    onConfiguredFitChanged: applyDebounce.restart()

    Component.onCompleted: {
        applyDebounce.restart();
        resizeDebounce.restart();
    }
}
