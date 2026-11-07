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
        page.paint = {
            "kind": "fill",
            "radius": 0.05,
            "falloff": 0.5,
            "by": 0.2,
            "second": false
        };
        paint.add(150, 20);
        paint.lay();
        if (strokes("fills").length !== 1 || strokes("fills")[0].by !== 1 || strokes("strokes").length !== 1)
            throw new Error("depth: a fill mark was not kept: " + JSON.stringify(page.edit.depth));
        page.paint = Object.assign({}, page.paint, {
            "second": true
        });
        paint.add(20, 80);
        paint.lay();
        if (strokes("fills")[1].by !== -1)
            throw new Error("depth: a protected mark was not kept: " + JSON.stringify(strokes("fills")));
        page.editor.undo();
        if (strokes("fills").length !== 1 || strokes("strokes").length !== 1)
            throw new Error("depth: undo did not remove one mark");
        const cuts = Qt.createComponent("EditDepthCuts.qml").createObject(page, {
                "page": page,
                "width": 300
            });
        cuts.put([0.2, 0.6]);
        if (JSON.stringify(page.edit.depth.cuts) !== "[0.2,0.6]")
            throw new Error("depth: the cuts were not kept: " + JSON.stringify(page.edit.depth));
        cuts.move(0, 0.3 * cuts.wide);
        if (Math.abs(page.cutsLive[0] - 0.3) > 1e-6 || page.cutsLive[1] !== 0.6)
            throw new Error("depth: a dragged handle did not show its cut: " + JSON.stringify(page.cutsLive));
        cuts.release();
        if (page.cutsLive !== null || JSON.stringify(page.edit.depth.cuts) !== "[0.3,0.6]")
            throw new Error("depth: releasing a handle did not keep the cuts: " + JSON.stringify(page.edit.depth));
        cuts.put([]);
        if (page.edit.depth.cuts !== undefined)
            throw new Error("depth: going back to automatic kept the cuts");
        const compare = Qt.createComponent("EditDepthCompare.qml").createObject(page, {
                "page": page
            });
        if (compare.names.length !== compare.count + 1 || compare.names[0] !== App.tr("None"))
            throw new Error("depth: the layer picker lists " + JSON.stringify(compare.names));
        page.paint = null;
        paint.destroy();
        cuts.destroy();
        compare.destroy();
    }
}
