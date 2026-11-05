import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    id: check

    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function exercise() {
        const steps = page.view.history.done.length;
        const pointer = "/scene/layers/0/opacity";
        page.run("timeline", {
            "id": "timing_check",
            "add": true,
            "duration": 4,
            "loop": "repeat"
        });
        page.run("keyframe", {
            "pointer": pointer,
            "time": 0,
            "value": 1,
            "timeline": "timing_check"
        });
        page.run("keyframe", {
            "pointer": pointer,
            "time": 3,
            "value": 0.5,
            "base": 0
        });
        page.run("keyframe", {
            "pointer": pointer,
            "time": 0,
            "out_handle": [1, 0]
        });
        page.run("timeline", {
            "id": "timing_check",
            "to": "timing_moved"
        });
        if (page.editor.error !== "")
            throw new Error("timing: " + page.editor.error);
        const keys = read(pointer + "/keys") ?? [];
        if (read(pointer + "/timeline") !== "timing_moved" || read(pointer + "/base") !== 0)
            throw new Error("timing: the keyframes do not follow the renamed timeline or lack their base");
        if (keys[0]?.[2]?.out?.[0] !== 1)
            throw new Error("timing: the Bézier handle of the first keyframe was not kept");
        if (page.view.history.done.length !== steps + 5)
            throw new Error("timing: a timing action did not take exactly one undo step");
        for (let k = 0; k < 5; k++)
            page.run("undo");
    }
}
