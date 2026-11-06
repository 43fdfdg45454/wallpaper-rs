import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: trial

    property string uid
    property string output
    property var strip: null
    property var stream: null
    property bool missing: false
    property string problem

    function start(uid) {
        stop();
        trial.uid = uid;
        output = "preview:page-lights-" + Date.now();
        strip = null;
        problem = "";
        missing = false;
        open();
        const asked = output;
        App.query("RequestStream", [asked, 640, 360], "Daemon1", reply => trial && trial.output === asked && trial.attach(reply), error => trial && (trial.problem = error.message));
    }

    function attach(reply) {
        const found = typeof reply === "string" ? JSON.parse(reply) : reply;
        App.act("Apply", [uid, output, "", ""], "Library1", () => {}, error => trial && (trial.problem = error.message));
        if (App.checking)
            return;
        try {
            stream = Qt.createQmlObject("import QtQuick; import org.kde.pipewire as PipeWire; PipeWire.PipeWireSourceItem { anchors.fill: parent }", screen);
            if (found[1] > 0 && stream.objectSerial !== undefined)
                stream.objectSerial = found[1];
            else
                stream.nodeId = found[0];
        } catch (error) {
            missing = true;
        }
    }

    function stop() {
        stream?.destroy();
        stream = null;
        if (output !== "")
            App.act("ReleaseStream", [output], "Daemon1");
        output = "";
    }

    title: App.tr("The page and its lights")
    preferredWidth: Kirigami.Units.gridUnit * 30
    standardButtons: Kirigami.Dialog.Close
    onClosed: stop()

    Timer {
        interval: 100
        repeat: true
        running: trial.opened && trial.output !== ""
        onTriggered: App.query("GetStrip", [trial.output], "Rgb1", reply => trial && (trial.strip = reply), () => trial && (trial.strip = null))
    }

    ColumnLayout {
        Rectangle {
            id: screen

            Layout.fillWidth: true
            Layout.preferredHeight: width * 9 / 16
            color: "black"

            QQC2.Label {
                anchors.centerIn: parent
                visible: trial.missing
                color: "white"
                text: App.tr("Install the KPipeWire QML module to see the page here.")
            }
        }

        QQC2.Label {
            text: trial.strip ? App.tr("What the lights get, %1 × %2:", trial.strip[0], trial.strip[1]) : App.tr("The page has sent no colors to the lights yet; pages that call window.wprs.rgb.setImage show them here.")
        }

        Grid {
            visible: trial.strip !== null
            columns: Math.max(1, trial.strip?.[0] ?? 1)
            spacing: 1

            Repeater {
                model: trial.strip ? trial.strip[0] * trial.strip[1] : 0

                delegate: Rectangle {
                    required property int index

                    width: Kirigami.Units.gridUnit * 0.4
                    height: width
                    color: Qt.rgba(trial.strip[2][index * 3] / 255, trial.strip[2][index * 3 + 1] / 255, trial.strip[2][index * 3 + 2] / 255, 1)
                }
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: trial.problem ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.disabledTextColor
            text: trial.problem || App.tr("Here window.wprs.rgb.setImage returns true and its colors are only shown, never sent. On the desktop it returns false and sends nothing until RGB lighting is on and you allow it for this package.")
        }
    }
}
