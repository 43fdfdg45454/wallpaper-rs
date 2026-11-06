import QtQuick

QtObject {
    required property var page

    function look(template) {
        page.editor.look = template ? JSON.stringify({
            "template": template
        }) : "";
        page.editor.seek(page.clock, false);
    }

    function exercise() {
        const scene = page.mesh.pointer("puppet");
        page.run("duplicate_layer", {
            "id": "puppet",
            "template": true
        });
        look("puppet_template");
        const root = "/scene/templates/puppet_template";
        if ((page.view.layers ?? [])[0]?.pointer !== root || page.mesh.pointer("puppet") !== root)
            throw new Error("the layers panel does not list the template shown alone: " + JSON.stringify(page.view.layers));
        if (!(page.view.gizmos ?? []).every(g => g.pointer.startsWith(root)))
            throw new Error("the canvas handles do not belong to the template: " + JSON.stringify(page.view.gizmos));
        const chains = page.mesh.ask("puppet", {
            "handles": true
        })?.ik ?? [];
        page.run("ik", {
            "layer": "puppet",
            "index": 0,
            "remove": true
        });
        const kept = JSON.parse(page.editor.value(root + "/ik") || "null") ?? [];
        if (chains.length === 0 || kept.length !== chains.length - 1)
            throw new Error("a chain inside the template was not edited there: " + JSON.stringify(kept));
        if ((JSON.parse(page.editor.value(scene + "/ik") || "null") ?? []).length !== chains.length)
            throw new Error("an edit inside the template changed the scene layer");
        look("");
        page.run("undo");
    }
}
