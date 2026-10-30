import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: scene

    required property var daemon
    required property string output
    property var props: ({})
    property var inputs: ({})
    property var folded: ({})
    property string problem: ""
    readonly property string packageId: props.package ?? ""
    readonly property var uses: (inputs.shown ?? {})[output] ?? {}
    readonly property var granted: uses.granted ?? []
    readonly property var fields: (props.fields ?? []).filter(field => field.visible && !scene.closed(field.group))

    function load() {
        daemon.query("GetSceneProperties", [output], "Daemon1", reply => {
            if (!scene)
                return;
            const fresh = Object.assign({}, scene.folded);
            (reply.fields ?? []).filter(f => f.kind === "group" && !((reply.package + "/" + f.key) in fresh)).forEach(f => fresh[reply.package + "/" + f.key] = f.collapsed);
            scene.folded = fresh;
            scene.props = reply;
        }, () => scene && (scene.props = ({})));
        daemon.query("GetInputs", [], "Inputs1", reply => scene && (scene.inputs = reply));
    }

    function closed(group) {
        return group !== "" && folded[packageId + "/" + group] === true;
    }

    function listed(map, input) {
        return ((map ?? {})[uses.scene] ?? []).includes(input);
    }

    function decide(input, allow) {
        problem = "";
        const failed = error => {
            if (!String(error.name).endsWith(".Unconfirmed") && !scene.listed(scene.inputs.quiet, input))
                scene.problem = error.message;
            scene.load();
        };
        daemon.act(allow ? "GrantPackage" : "Revoke", allow ? [uses.scene, uses.key, input] : [uses.scene, input], "Inputs1", () => scene && scene.load(), failed);
    }

    function set(key, value) {
        daemon.act("SetSceneProperty", [output, key, JSON.stringify(value)]);
    }

    Component.onCompleted: load()

    Connections {
        target: scene.daemon

        function onUpdated(topic) {
            if (["state", "inputs", "profiles"].includes(topic))
                reload.restart();
        }
    }

    Timer {
        id: reload

        interval: 300
        onTriggered: scene.load()
    }

    Consent {
        Layout.fillWidth: true
        daemon: scene.daemon
        inputs: scene.inputs
        output: scene.output
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: scene.problem !== ""
        type: Kirigami.MessageType.Error
        text: scene.problem
    }

    Repeater {
        model: [["audio", scene.daemon.tr("Follow the sound that plays"), scene.daemon.tr("The sound is turned into levels on this computer while the scene is shown; nothing is recorded, kept or sent, and the desktop may show a microphone indicator.")], ["pointer", scene.daemon.tr("Follow the pointer"), scene.daemon.tr("Only the latest pointer position over this screen reaches the scene; positions are never kept and keys are never read.")], ["clicks", scene.daemon.tr("Take clicks and taps"), scene.daemon.tr("Only real clicks and taps on the wallpaper of this screen reach the scene; they are never kept and keys are never read.")]].filter(input => scene.uses[input[0]] === true)

        delegate: ColumnLayout {
            id: input

            required property var modelData
            readonly property bool asking: scene.listed(scene.inputs.asking, modelData[0])
            readonly property bool quiet: scene.listed(scene.inputs.quiet, modelData[0])

            Layout.fillWidth: true

            Connections {
                target: scene

                function onInputsChanged() {
                    toggle.checked = Qt.binding(() => scene.granted.includes(input.modelData[0]));
                }
            }

            QQC2.Switch {
                id: toggle

                text: input.modelData[1]
                checked: scene.granted.includes(input.modelData[0])
                enabled: !input.asking && (!input.quiet || checked)
                onToggled: scene.decide(input.modelData[0], checked)
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: input.asking || input.quiet
                text: input.asking ? scene.daemon.tr("Confirm it in the desktop notification.") : scene.daemon.tr("The notification was closed without an answer; try again in a few minutes.")
                wrapMode: Text.Wrap
            }

            QQC2.Label {
                Layout.fillWidth: true
                text: input.modelData[2]
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
            }
        }
    }

    QQC2.Switch {
        id: screenClicks

        visible: scene.uses.clicks === true
        text: scene.daemon.tr("Let clicks and taps on this screen reach its wallpaper")
        checked: (scene.inputs.clicks ?? []).includes(scene.output)
        onToggled: scene.daemon.act("SetClicks", [scene.output, checked], "Inputs1", () => scene && scene.load(), error => {
            if (!scene)
                return;
            scene.problem = error.message;
            scene.load();
        })

        Connections {
            target: scene

            function onInputsChanged() {
                screenClicks.checked = Qt.binding(() => (scene.inputs.clicks ?? []).includes(scene.output));
            }
        }
    }

    ScenePresets {
        Layout.fillWidth: true
        visible: scene.packageId !== ""
        daemon: scene.daemon
        output: scene.output
        props: scene.props
    }

    Repeater {
        model: scene.fields

        delegate: SceneField {
            Layout.fillWidth: true
            daemon: scene.daemon
            output: scene.output
            folded: scene.closed(modelData.key)
            onEdited: (key, value) => scene.set(key, value)
            onFold: (key, on) => {
                const next = Object.assign({}, scene.folded);
                next[scene.packageId + "/" + key] = on;
                scene.folded = next;
            }
        }
    }
}
