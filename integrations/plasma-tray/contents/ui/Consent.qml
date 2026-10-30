import QtQuick
import org.kde.kirigami as Kirigami

Kirigami.InlineMessage {
    id: consent

    required property var daemon
    property var inputs: ({})
    property string output: ""
    property bool unconfirmed: false
    property string problem: ""
    readonly property bool pending: listed(inputs.asking) || unconfirmed && inputs.asking === undefined
    readonly property bool quiet: listed(inputs.quiet)
    readonly property string asked: [ask.scene, ask.key].concat(ask.inputs).join(" ")
    readonly property var explained: ({
            "audio": [daemon.tr("Follow the sound that plays"), daemon.tr("The sound is turned into levels on this computer while the scene is shown; nothing is recorded, kept or sent, and the desktop may show a microphone indicator.")],
            "pointer": [daemon.tr("Follow the pointer"), daemon.tr("Only the latest pointer position over this screen reaches the scene; positions are never kept and keys are never read.")],
            "clicks": [daemon.tr("Take clicks and taps"), daemon.tr("Only real clicks and taps on the wallpaper of this screen reach the scene; they are never kept and keys are never read.")]
        })
    readonly property var ask: {
        const shown = inputs.shown ?? {};
        const decided = (map, scene, input) => ((map ?? {})[scene] ?? []).includes(input);
        const open = (s, input) => s[input] === true && !(s.granted ?? []).includes(input) && !decided(inputs.declined, s.scene, input);
        for (const name of Object.keys(shown).filter(name => consent.output === "" || name === consent.output)) {
            const list = ["audio", "pointer", "clicks"].filter(input => open(shown[name], input));
            if (list.length > 0)
                return {
                    "screen": name,
                    "scene": shown[name].scene,
                    "key": shown[name].key ?? "",
                    "title": shown[name].title || shown[name].scene,
                    "inputs": list
                };
        }
        return {
            "screen": "",
            "scene": "",
            "key": "",
            "title": "",
            "inputs": []
        };
    }

    function listed(map) {
        return ask.inputs.some(input => ((map ?? {})[ask.scene] ?? []).includes(input));
    }

    function answer(allow) {
        problem = "";
        ask.inputs.forEach(input => daemon.act(allow ? "GrantPackage" : "Revoke", allow ? [ask.scene, ask.key, input] : [ask.scene, input], "Inputs1", () => {}, error => consent.failed(error)));
        if (allow && ask.inputs.includes("clicks") && !(inputs.clicks ?? []).includes(ask.screen))
            daemon.act("SetClicks", [ask.screen, true], "Inputs1", () => {}, error => consent.failed(error));
    }

    function failed(error) {
        if (String(error.name).endsWith(".Unconfirmed"))
            unconfirmed = true;
        else if (!quiet)
            problem = error.message;
    }

    onAskedChanged: {
        unconfirmed = false;
        problem = "";
    }
    visible: ask.inputs.length > 0
    type: problem ? Kirigami.MessageType.Error : Kirigami.MessageType.Information
    text: [daemon.tr("The scene %1 asks for your permission:", String(ask.title).replace(/</g, "‹").replace(/>/g, "›"))].concat(ask.inputs.map(input => explained[input].join(": "))).concat(pending ? [daemon.tr("Confirm it in the desktop notification.")] : []).concat(quiet && !pending ? [daemon.tr("The notification was closed without an answer; try again in a few minutes.")] : []).concat(problem ? [problem] : []).join("\n")
    actions: [
        Kirigami.Action {
            text: consent.daemon.tr("Allow")
            icon.name: "dialog-ok-apply"
            enabled: !consent.pending && !consent.quiet
            onTriggered: consent.answer(true)
        },
        Kirigami.Action {
            text: consent.daemon.tr("Not now")
            icon.name: "dialog-cancel"
            onTriggered: consent.answer(false)
        }
    ]
}
