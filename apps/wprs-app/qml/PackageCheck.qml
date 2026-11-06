import QtQuick

QtObject {
    required property var page

    function has(path, role) {
        return (page.view.files ?? []).some(file => file.path === path && (role === undefined || file.role === role));
    }

    function exercise() {
        if (page.kind !== "web" || !has("index.html", "entry") || !has("app.js"))
            throw new Error("the web starter has no page: " + JSON.stringify(page.view.files));
        page.manifestEdit("add_file", {
            "file": page.editor.folder + "/app.js",
            "to": "lib"
        });
        if (!has("lib/app.js"))
            throw new Error("a file was not added to the page");
        page.change("/manifest/entry", "missing.html");
        if (!(page.view.problems ?? []).some(problem => problem.pointer === "/manifest/entry"))
            throw new Error("a missing entry page is no problem");
        page.run("undo");
        page.editor.save();
        if (page.editor.dirty || page.editor.error !== "")
            throw new Error("the page was not saved: " + page.editor.error);
    }
}
