import QtQuick

QtObject {
    required property var page

    function manifest() {
        const field = key => page.fields.find(item => item.key === key) ?? {};
        page.editor.select("/manifest");
        if (field("title").widget !== "texts" || field("description").widget !== "markup" || field("authors").widget !== "lines")
            throw new Error("the manifest has no text editors");
        page.run("translate", {
            "pointer": "/manifest/title",
            "language": "es",
            "text": "Prueba"
        });
        if (!field("title").options.some(row => row.language === "es" && row.text === "Prueba"))
            throw new Error("the title has no Spanish text");
        for (const language of ["es", "de"])
            page.run("translate", {
                "pointer": "/manifest/title",
                "language": language,
                "add": true
            });
        const rows = field("title").options;
        if (!rows.some(row => row.language === "es" && row.text === "Prueba") || !rows.some(row => row.language === "de" && row.text === rows[0].text))
            throw new Error("adding a language changed a text");
        if (field("default_locale").widget !== "ref:language" || !field("default_locale").required)
            throw new Error("the default language can be reset");
        page.change("/manifest/content", {
            "rating": "mature",
            "descriptors": ["violence"]
        });
        page.change("/manifest/photosensitive", true);
        page.change("/manifest/inputs", ["media"]);
        page.run("set", {
            "pointer": "/manifest/origin",
            "value": {
                "kind": "derived",
                "derived_from": [
                    {
                        "author": "Someone"
                    }
                ]
            }
        });
        page.run("set", {
            "pointer": "/manifest/links",
            "value": {
                "homepage": "https://example.org"
            }
        });
        if (!field("inputs").value.includes("pointer") || !field("inputs").fixed.includes("pointer"))
            throw new Error("an input the scene uses was dropped");
        page.editor.select("/manifest/requires");
        if (field("features").widget !== "computed" || !field("features").value.includes("inputs.media") || !field("features").options.includes("locale_files"))
            throw new Error("the features do not follow the inputs");
        page.run("set", {
            "pointer": "/manifest/schema",
            "value": "wallpaper-rs.wpak/1"
        });
        page.run("upgrade");
        page.editor.select("/manifest/content");
        if (!field("descriptors").options.includes("gore") || page.view.rating !== "mature" || page.view.v2 !== true)
            throw new Error("the rating did not stay");
    }

    function exercise() {
        for (const kind of page.view.starters) {
            page.editor.create(page.folder + "-" + kind, kind, "");
            page.run("add_layer", {
                "kind": "text"
            });
            page.change("/scene/layers/0/opacity", 0.5);
            page.run("keyframe", {
                "pointer": "/scene/layers/0/opacity",
                "time": 1,
                "value": 0.25,
                "ease": "in_out_sine"
            });
            page.run("duplicate_layer", {
                "id": "text"
            });
            page.run("lock", {
                "id": "text",
                "locked": true
            });
            page.run("hide", {
                "id": "text_copy",
                "hidden": true
            });
            page.run("solo", {
                "id": "text"
            });
            page.run("group", {
                "ids": ["text_copy"]
            });
            page.run("undo");
            page.run("redo");
            page.run("set", {
                "pointer": "/scene/timelines",
                "value": [
                    {
                        "id": "intro",
                        "duration": 2
                    }
                ]
            });
            page.run("set", {
                "pointer": "/scene/layers/0/opacity/timeline",
                "value": "intro"
            });
            page.run("set", {
                "pointer": "/scene/layers/0/effects",
                "value": [
                    {
                        "effect": "vignette"
                    },
                    {
                        "effect": "film_grain"
                    }
                ]
            });
            page.run("move", {
                "from": "/scene/layers/0/effects/1",
                "to": "/scene/layers/0/effects/0"
            });
            page.editor.select("/scene/layers/0/effects/0");
            if (page.editor.count("/scene/layers/0/effects") !== 2 || !page.fields.some(field => field.key === "effect" && field.value === "film_grain"))
                throw new Error(kind + ": the effects did not move");
            page.editor.select("/scene/layers/0/opacity");
            if (!page.fields.some(field => field.key === "timeline" && field.value === "intro"))
                throw new Error(kind + ": the keyframes do not show their timeline");
            const steps = page.view.history.done.length;
            page.change("/scene/canvas/background", "#101010", true);
            page.change("/scene/canvas/background", "#202020", true);
            if (page.view.history.done.length !== steps + 1)
                throw new Error(kind + ": a drag took more than one undo step");
            page.editor.select("/scene/layers/0");
            if (kind !== "empty" && !page.fields.some(field => field.widget === "stops"))
                throw new Error(kind + ": gradient stops have no editor");
            if (kind === "particles") {
                page.editor.select("/scene/particles/flakes/emitters/0");
                if (!page.fields.some(field => field.widget === "points"))
                    throw new Error(kind + ": the emitter box has no editor");
            }
            if (kind === "parallax") {
                manifest();
                Qt.createComponent("SceneSettingsCheck.qml").createObject(page, {
                    "page": page
                }).exercise();
            }
            page.jump("/scene/layers/0/opacity");
            page.editor.seek(0.5, false);
            if (page.editor.error !== "" || page.view.problems.some(problem => !problem.warning))
                throw new Error(kind + ": " + (page.editor.error || JSON.stringify(page.view.problems)));
        }
        page.editor.save();
        page.tabs.currentIndex = 2;
        page.editor.select("/manifest");
        page.tabs.currentIndex = 1;
        page.builder.open();
        page.builder.close();
    }
}
