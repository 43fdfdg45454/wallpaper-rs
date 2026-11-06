import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function exercise() {
        const files = Qt.createComponent("SceneLayerFiles.qml").createObject(page, {
            "owner": {
                "page": page,
                "current": null
            }
        });
        if (files.count !== 4)
            throw new Error("files: the menu offers " + files.count + " kinds of file, not 4");
        for (const kind of ["image", "video", "sound", "mesh"]) {
            files.kind = kind;
            if (!/\(\*\.[a-z0-9]+/.test(files.dialog.nameFilters[0] ?? ""))
                throw new Error("files: no file filter for " + kind);
        }
        files.destroy();
    }
}
