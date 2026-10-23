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
    readonly property string packageId: props.package ?? ""
    readonly property var uses: (inputs.shown ?? {})[output] ?? {}
    readonly property var granted: (inputs.grants ?? {})[packageId] ?? []
    readonly property var fields: (props.fields ?? []).filter(field => field.visible && !scene.closed(field.group))

    function load() {
        daemon.query("GetSceneProperties", [output], "Daemon1", reply => {
            const fresh = Object.assign({}, scene.folded);
            (reply.fields ?? []).filter(f => f.kind === "group" && !((reply.package + "/" + f.key) in fresh)).forEach(f => fresh[reply.package + "/" + f.key] = f.collapsed);
            scene.folded = fresh;
            scene.props = reply;
        }, () => scene.props = ({}));
        daemon.query("GetInputs", [], "Inputs1", reply => scene.inputs = reply);
    }

    function closed(group) {
        return group !== "" && folded[packageId + "/" + group] === true;
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

    Repeater {
        model: [["audio", scene.daemon.tr("Follow the sound that plays"), scene.daemon.tr("The sound is turned into levels on this computer while the scene is shown; nothing is recorded, kept or sent, and the desktop may show a microphone indicator.")], ["pointer", scene.daemon.tr("Follow the pointer"), scene.daemon.tr("Only the latest pointer position over this screen reaches the scene; positions are never kept and keys are never read.")]].filter(input => scene.uses[input[0]] === true)

        delegate: ColumnLayout {
            id: input

            required property var modelData

            Layout.fillWidth: true

            QQC2.Switch {
                text: input.modelData[1]
                checked: scene.granted.includes(input.modelData[0])
                onToggled: checked ? scene.daemon.act("Grant", [scene.packageId, input.modelData[0]], "Inputs1") : scene.daemon.act("Revoke", [scene.packageId, input.modelData[0]], "Inputs1")
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
