import QtQuick

QtObject {
    required property var page

    function fail(text) {
        throw new Error("properties: " + text);
    }

    function exercise() {
        const panel = page.settings;
        page.tabs.currentIndex = 2;
        panel.add("combo");
        const combo = page.view.properties.find(p => p.id === "combo_1");
        if (page.editor.selected !== "/manifest/properties/combo_1" || combo?.label !== "combo_1")
            fail("a new property is not selected or carries a label in the language of the desktop");
        panel.tools.row = combo;
        panel.tools.rename("weather");
        if (page.editor.selected !== "/manifest/properties/weather" || !page.fields.some(f => f.key === "options"))
            fail("the inspector stayed on the old id after a rename");
        page.run("set", {
                "pointer": "/scene/layers/0/visible",
                "value": {
                    "prop": "weather",
                    "map": {
                        "option_1": true
                    }
                }
            });
        panel.tools.row = page.view.properties.find(p => p.id === "weather");
        panel.tools.remove();
        if (!page.editor.error.includes("/scene/layers/0/visible") || page.editor.selected !== "/manifest/properties/weather")
            fail("a property that a binding names was removed");
        page.run("undo");
        panel.tools.remove();
        if (page.editor.selected !== "/manifest" || page.view.properties.some(p => p.id === "weather"))
            fail("the inspector stayed on a removed property");
        for (let i = 0; i < 24; i++)
            panel.addPreset();
        const last = page.view.presets[page.view.presets.length - 1];
        if (page.editor.selected !== last.pointer || page.fields.find(f => f.key === "id")?.value !== last.id)
            fail("adding a preset selected another one");
        if (panel.list.footerItem?.parent !== panel.list.contentItem)
            fail("the presets do not scroll with the properties");
    }
}
