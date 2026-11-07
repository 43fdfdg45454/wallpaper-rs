import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function strokes(list) {
        return (page.edit.depth ?? {})[list] ?? [];
    }

    function exercise() {
        const paint = Qt.createComponent("EditDepthPaint.qml").createObject(page, {
                "page": page,
                "width": 200,
                "height": 100
            });
        page.run("depth_brush", {
                "clear": true
            });
        page.run("fill_mask", {
                "clear": true
            });
        page.paint = {
            "kind": "depth",
            "radius": 0.05,
            "falloff": 0.5,
            "by": 0.2,
            "second": true
        };
        paint.add(20, 10);
        paint.add(100, 50);
        paint.add(100.5, 50.5);
        paint.lay();
        const first = strokes("strokes");
        if (first.length !== 1 || first[0].points.length !== 2 || first[0].by !== -0.2 || first[0].points[1][0] !== 0.5)
            throw new Error("depth: a stroke of the brush was not kept as one step: " + JSON.stringify(first));
        page.paint = null;
        paint.destroy();
    }
}
