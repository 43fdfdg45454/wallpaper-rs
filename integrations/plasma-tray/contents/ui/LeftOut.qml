import QtQml

QtObject {
    function text(line) {
        const head = "the effect ", tail = " was left out: ";
        const at = line.indexOf(tail);
        if (!line.startsWith(head) || at < 0)
            return line;
        const why = line.slice(at + tail.length);
        const reasons = [["it is too slow for this graphics card", i18n("it is too slow for this graphics card")], ["it stopped the graphics card or the program twice", i18n("it stopped the graphics card or the program twice")], ["the graphics card or the program stopped twice", i18n("the graphics card or the program stopped twice while it built or drew this effect")], ["the graphics card or the program stopped", i18n("the graphics card or the program stopped while it drew this effect")], ["it kept the graphics card busy", i18n("it kept the graphics card busy for too long")], ["its shader could not be checked", i18n("its shader could not be checked")], ["the effects of the scene would do more than", i18n("the effects of the scene would do more work than this graphics card allows")], ["the scene has more than", i18n("the scene has more than 16 effects that keep a state")], ["its state textures would take", i18n("its state textures would take too much memory")], ["its textures would take more than", i18n("its textures would take too much memory")]];
        const found = reasons.find(r => why.startsWith(r[0]));
        if (!found || found[1].startsWith(found[0]))
            return line;
        return i18n("The effect %1 was left out: %2", line.slice(head.length, at), found[1]);
    }

    function privacy(exposes, sandbox) {
        const words = {
            "zone": i18n("your time zone"),
            "language": i18n("your language"),
            "gpu": i18n("the model of your graphics card"),
            "storage": i18n("data it keeps on this computer"),
            "network": i18n("the network, to reach the sites it names")
        };
        const used = exposes.length ? i18n("This web wallpaper uses %1. You can restrict each of them in its settings in the app.", exposes.map(name => words[name] ?? name).join(", ")) : "";
        return [used, sandbox === "reduced" ? i18n("Its web engine runs without part of its sandbox on this computer.") : ""].filter(t => t).join(" ");
    }

    function level(info, state) {
        const reasons = {
            "device": i18n("the most this graphics card allows"),
            "cost": i18n("lowered because the scene is heavy for this graphics card"),
            "battery": i18n("lowered on battery"),
            "saver": i18n("lowered while saving power"),
            "heat": i18n("lowered while the computer is hot"),
            "slow": i18n("lowered because its frames took too long on the graphics card")
        };
        const quality = i18n("Quality %1 of 3", info.quality.slice(1));
        const tier = String(state?.tier ?? "").slice(1);
        const card = !tier ? "" : state.tier_measured ? i18n("graphics card class %1 of 3, measured", tier) : i18n("graphics card class %1 of 3, from its model", tier);
        return [reasons[info.quality_reason] ? quality + ": " + reasons[info.quality_reason] : quality, card].filter(t => t).join(" · ");
    }
}
