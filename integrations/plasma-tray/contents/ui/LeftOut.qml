import QtQml

QtObject {
    function text(line) {
        const head = "the effect ", tail = " was left out: "
        const at = line.indexOf(tail)
        if (!line.startsWith(head) || at < 0)
            return line
        const why = line.slice(at + tail.length)
        const reasons = [
            ["it is too slow for this graphics card", i18n("it is too slow for this graphics card")],
            ["it stopped the graphics card or the program twice", i18n("it stopped the graphics card or the program twice")],
            ["the graphics card or the program stopped twice", i18n("the graphics card or the program stopped twice while it built or drew this effect")],
            ["the graphics card or the program stopped", i18n("the graphics card or the program stopped while it drew this effect")],
            ["it kept the graphics card busy", i18n("it kept the graphics card busy for too long")],
            ["its shader could not be checked", i18n("its shader could not be checked")],
            ["the effects of the scene would do more than", i18n("the effects of the scene would do more work than this graphics card allows")],
            ["the scene has more than", i18n("the scene has more than 16 effects that keep a state")],
            ["its state textures would take", i18n("its state textures would take too much memory")],
            ["its textures would take more than", i18n("its textures would take too much memory")]
        ]
        const found = reasons.find(r => why.startsWith(r[0]))
        if (!found || found[1].startsWith(found[0]))
            return line
        return i18n("The effect %1 was left out: %2", line.slice(head.length, at), found[1])
    }
}
