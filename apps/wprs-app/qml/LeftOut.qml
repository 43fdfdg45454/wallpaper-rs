import QtQml

QtObject {
    required property var daemon

    function text(line) {
        const head = "the effect ", tail = " was left out: "
        const at = line.indexOf(tail)
        if (!line.startsWith(head) || at < 0)
            return line
        const why = line.slice(at + tail.length)
        const reasons = [
            ["it is too slow for this graphics card", daemon.tr("it is too slow for this graphics card")],
            ["it stopped the graphics card or the program twice", daemon.tr("it stopped the graphics card or the program twice")],
            ["the graphics card or the program stopped twice", daemon.tr("the graphics card or the program stopped twice while it built or drew this effect")],
            ["the graphics card or the program stopped", daemon.tr("the graphics card or the program stopped while it drew this effect")],
            ["it kept the graphics card busy", daemon.tr("it kept the graphics card busy for too long")],
            ["its shader could not be checked", daemon.tr("its shader could not be checked")],
            ["the effects of the scene would do more than", daemon.tr("the effects of the scene would do more work than this graphics card allows")],
            ["the scene has more than", daemon.tr("the scene has more than 16 effects that keep a state")],
            ["its state textures would take", daemon.tr("its state textures would take too much memory")],
            ["its textures would take more than", daemon.tr("its textures would take too much memory")]
        ]
        const found = reasons.find(r => why.startsWith(r[0]))
        if (!found || found[1].startsWith(found[0]))
            return line
        return daemon.tr("The effect %1 was left out: %2").arg(line.slice(head.length, at)).arg(found[1])
    }
}
