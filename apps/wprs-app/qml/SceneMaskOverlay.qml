import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: overlay

    required property var page
    required property real ratio
    readonly property var found: page.editor.selected.match(/^(.*\/layers\/\d+)\/effects\/(\d+)(\/mask(\/.*)?)?$/)
    readonly property var gizmo: found ? (page.view.gizmos ?? []).find(g => g.pointer === found[1]) ?? null : null
    readonly property var owner: found ? (page.view.layers ?? []).find(l => l.pointer === found[1]) ?? null : null
    readonly property var mask: gizmo ? (page.view, JSON.parse(page.editor.value(found[1] + "/effects/" + found[2] + "/mask") || "null")) : null
    readonly property string texture: typeof mask === "string" ? mask : mask?.texture ?? ""
    readonly property string kind: mask?.shape ? Object.keys(mask.shape)[0] : ""
    readonly property var spec: kind ? mask.shape[kind] : ({})
    readonly property var box: kind === "line" ? [Math.abs((spec.to ?? [0, 0])[0]) + (spec.width ?? 0), Math.abs((spec.to ?? [0, 0])[1]) + (spec.width ?? 0)] : ["polygon", "star"].includes(kind) ? [spec.size ?? 0, spec.size ?? 0] : spec.size ?? [0, 0]
    readonly property var source: typeof mask?.layer === "string" ? (page.view.gizmos ?? []).find(g => g.id === mask.layer) ?? null : null

    function put(fields) {
        page.run("effects", {
                "mask": Object.assign({
                        "layer": owner.id,
                        "index": Number(found[2])
                    }, fields)
            }, true);
    }

    function sized(local) {
        const at = [Math.abs(local[0]) * 2, Math.abs(local[1]) * 2].map(v => Math.max(1, Math.round(v)));
        const next = Object.assign({}, spec);
        if (kind === "line")
            next.to = local.map(v => Math.round(v * 2));
        else if (["polygon", "star"].includes(kind))
            next.size = Math.max(at[0], at[1]);
        else
            next.size = at;
        put({
                "shape": {
                    [kind]: next
                }
            });
    }

    visible: gizmo !== null && mask !== null

    SceneMaskFrame {
        overlay: overlay
        shown: overlay.gizmo
    }

    SceneMaskFrame {
        overlay: overlay
        shown: overlay.source
        outlined: true
    }
}
