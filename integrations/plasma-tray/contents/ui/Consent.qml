import QtQuick
import org.kde.kirigami as Kirigami

Kirigami.InlineMessage {
    id: consent

    required property var daemon
    property var inputs: ({})
    property string output: ""
    readonly property var explained: ({
            "audio": [daemon.tr("Follow the sound that plays"), daemon.tr("The sound is turned into levels on this computer while the scene is shown; nothing is recorded, kept or sent, and the desktop may show a microphone indicator.")],
            "pointer": [daemon.tr("Follow the pointer"), daemon.tr("Only the latest pointer position over this screen reaches the scene; positions are never kept and keys are never read.")]
        })
    readonly property var ask: {
        const shown = inputs.shown ?? {};
        const decided = (map, scene, input) => ((map ?? {})[scene] ?? []).includes(input);
        const open = (s, input) => s[input] === true && !decided(inputs.grants, s.scene, input) && !decided(inputs.declined, s.scene, input);
        for (const name of Object.keys(shown).filter(name => consent.output === "" || name === consent.output)) {
            const list = ["audio", "pointer"].filter(input => open(shown[name], input));
            if (list.length > 0)
                return {
                    "scene": shown[name].scene,
                    "title": shown[name].title || shown[name].scene,
                    "inputs": list
                };
        }
        return {
            "scene": "",
            "title": "",
            "inputs": []
        };
    }

    function answer(allow) {
        ask.inputs.forEach(input => daemon.act(allow ? "Grant" : "Revoke", [ask.scene, input], "Inputs1"));
    }

    visible: ask.inputs.length > 0
    type: Kirigami.MessageType.Information
    text: [daemon.tr("The scene %1 asks for your permission:", String(ask.title).replace(/</g, "‹").replace(/>/g, "›"))].concat(ask.inputs.map(input => explained[input].join(": "))).join("\n")
    actions: [
        Kirigami.Action {
            text: consent.daemon.tr("Allow")
            icon.name: "dialog-ok-apply"
            onTriggered: consent.answer(true)
        },
        Kirigami.Action {
            text: consent.daemon.tr("Not now")
            icon.name: "dialog-cancel"
            onTriggered: consent.answer(false)
        }
    ]
}
