pragma Singleton
import QtQuick

QtObject {
    property string quality: "q3"
    property int restart: 0
    property var media: null
    readonly property var sample: ({
            "title": "Night Drive",
            "artist": "Example Artist",
            "album": "Example Album",
            "playing": true,
            "position": 65,
            "duration": 200
        })
    readonly property var levels: ["q0", "q1", "q2", "q3"]

    function preview(page) {
        const look = JSON.parse(page.editor.look || "{}");
        look.quality = quality;
        look.restart = restart;
        if (media)
            look.media = media;
        else
            delete look.media;
        page.editor.look = JSON.stringify(look);
        page.editor.seek(page.clock, page.editor.playing);
    }

    function again(page) {
        restart += 1;
        preview(page);
    }
}
