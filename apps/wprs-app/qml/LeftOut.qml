import QtQml

QtObject {
    required property var daemon

    function text(line) {
        const head = "the effect ", tail = " was left out: ";
        const at = line.indexOf(tail);
        if (!line.startsWith(head) || at < 0)
            return line;
        const why = line.slice(at + tail.length);
        const reasons = [["it is too slow for this graphics card", daemon.tr("it is too slow for this graphics card")], ["it stopped the graphics card or the program twice", daemon.tr("it stopped the graphics card or the program twice")], ["the graphics card or the program stopped twice", daemon.tr("the graphics card or the program stopped twice while it built or drew this effect")], ["the graphics card or the program stopped", daemon.tr("the graphics card or the program stopped while it drew this effect")], ["it kept the graphics card busy", daemon.tr("it kept the graphics card busy for too long")], ["its shader could not be checked", daemon.tr("its shader could not be checked")], ["the effects of the scene would do more than", daemon.tr("the effects of the scene would do more work than this graphics card allows")], ["the scene has more than", daemon.tr("the scene has more than 16 effects that keep a state")], ["its state textures would take", daemon.tr("its state textures would take too much memory")], ["its textures would take more than", daemon.tr("its textures would take too much memory")]];
        const found = reasons.find(r => why.startsWith(r[0]));
        if (!found || found[1].startsWith(found[0]))
            return line;
        return daemon.tr("The effect %1 was left out: %2", line.slice(head.length, at), found[1]);
    }

    function level(info, state) {
        const reasons = {
            "device": daemon.tr("the most this graphics card allows"),
            "cost": daemon.tr("lowered because the scene is heavy for this graphics card"),
            "battery": daemon.tr("lowered on battery"),
            "saver": daemon.tr("lowered while saving power"),
            "heat": daemon.tr("lowered while the computer is hot"),
            "slow": daemon.tr("lowered because its frames took too long on the graphics card"),
            "user": daemon.tr("chosen by you")
        };
        const quality = daemon.tr("Quality %1 of 3", info.quality.slice(1));
        const tier = String(state?.tier ?? "").slice(1);
        const card = !tier ? "" : state.tier_measured ? daemon.tr("graphics card class %1 of 3, measured", tier) : daemon.tr("graphics card class %1 of 3, from its model", tier);
        return [reasons[info.quality_reason] ? quality + ": " + reasons[info.quality_reason] : quality, card].filter(t => t).join(" · ");
    }

    function note(info) {
        const over = info.quality_budget > 0 && info.quality_cost > info.quality_budget;
        const auto = info.quality_auto ?? "";
        const left = (info.left_out ?? []).length;
        const kept = left ? daemon.tr("Effects left out to protect the graphics card stay out at every level: %1", left) : "";
        const cost = daemon.tr("This scene costs %1 units a frame at this level, more than the %2 this device is estimated to hold: frames may drop and battery use rise.", Number(info.quality_cost).toFixed(1), Number(info.quality_budget).toFixed(0));
        const would = auto === "stop" ? daemon.tr("Automatic quality would keep it still at its poster.") : daemon.tr("Automatic quality would lower it.");
        if (over)
            return [cost, would, kept].filter(t => t).join(" ");
        if (auto === "stop" && info.quality_reason === "user")
            return [daemon.tr("This scene draws more model triangles than this device holds: frames may drop and battery use rise. Automatic quality would keep it still at its poster."), kept].filter(t => t).join(" ");
        if (auto === "stop")
            return [daemon.tr("This scene is too heavy for this device, so automatic quality keeps it still at its poster. A fixed level plays it anyway, with fewer frames per second and more battery use."), kept].filter(t => t).join(" ");
        return "";
    }
}
