import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.FormLayout {
    id: panel

    required property var page
    required property string pointer
    readonly property var found: pointer.match(/^(.*\/layers\/\d+)\/effects\/(\d+)(\/mask(\/.*)?)?$/)
    readonly property var owner: found ? (page.view.layers ?? []).find(item => item.pointer === found[1]) ?? null : null
    readonly property int index: found ? Number(found[2]) : -1
    readonly property var mask: owner ? (page.view, JSON.parse(page.editor.value(found[1] + "/effects/" + found[2] + "/mask") || "null")) : null
    readonly property var gizmo: owner ? (page.view.gizmos ?? []).find(g => g.pointer === found[1]) ?? null : null
    readonly property string source: mask === null ? "" : typeof mask === "string" ? "texture" : ["texture", "layer", "shape"].find(k => mask[k] !== undefined) ?? ""
    readonly property string kind: source === "shape" ? Object.keys(mask.shape)[0] : ""
    readonly property var images: (page.view.assets ?? []).filter(a => a.kind === "image" || a.kind === "video").map(a => a.id)
    readonly property var others: (page.view.layers ?? []).filter(l => l.pointer !== (found ? found[1] : "") && l.type !== "sound").map(l => l.id)

    function put(fields, gesture) {
        page.run("effects", {
                "mask": Object.assign({
                        "layer": owner.id,
                        "index": index
                    }, fields)
            }, gesture === true);
    }

    function shaped(kind) {
        const box = gizmo ? gizmo.size.map(v => Math.round(v / 2)) : [200, 200];
        const side = Math.min(box[0], box[1]);
        return ({
                "rect": {
                    "rect": {
                        "size": box,
                        "radius": 0
                    }
                },
                "ellipse": {
                    "ellipse": {
                        "size": box
                    }
                },
                "ring": {
                    "ring": {
                        "size": box,
                        "width": Math.max(1, Math.round(side / 8))
                    }
                },
                "polygon": {
                    "polygon": {
                        "sides": 6,
                        "size": side
                    }
                },
                "star": {
                    "star": {
                        "points": 5,
                        "size": side,
                        "inner": 0.5
                    }
                },
                "line": {
                    "line": {
                        "to": [box[0], 0],
                        "width": Math.max(1, Math.round(side / 10))
                    }
                }
            })[kind];
    }

    visible: owner !== null

    QQC2.ComboBox {
        Kirigami.FormData.label: App.tr("Mask of the effect:")
        model: [App.tr("None"), App.tr("A picture"), App.tr("A layer"), App.tr("A shape")]
        currentIndex: ["", "texture", "layer", "shape"].indexOf(panel.source)
        onActivated: index => {
            if (index === 0)
                panel.put({
                        "off": true
                    });
            else if (index === 1 && panel.images.length > 0)
                panel.put({
                        "texture": panel.images[0]
                    });
            else if (index === 2 && panel.others.length > 0)
                panel.put({
                        "from": panel.others[0]
                    });
            else if (index === 3)
                panel.put({
                        "shape": panel.shaped("ellipse")
                    });
        }
    }

    QQC2.ComboBox {
        Kirigami.FormData.label: panel.source === "texture" ? App.tr("Picture:") : App.tr("Layer:")
        visible: panel.source === "texture" || panel.source === "layer"
        model: panel.source === "texture" ? panel.images : panel.others
        currentIndex: model.indexOf(panel.source === "texture" ? (typeof panel.mask === "string" ? panel.mask : panel.mask?.texture) : panel.mask?.layer)
        onActivated: index => panel.put(panel.source === "texture" ? {
                    "texture": panel.images[index]
                } : {
                    "from": panel.others[index]
                })
    }

    QQC2.ComboBox {
        Kirigami.FormData.label: App.tr("Shape:")
        visible: panel.source === "shape"
        model: ["rect", "ellipse", "ring", "polygon", "star", "line"].map(k => SceneWords.value(k))
        currentIndex: ["rect", "ellipse", "ring", "polygon", "star", "line"].indexOf(panel.kind)
        onActivated: index => panel.put({
                    "shape": panel.shaped(["rect", "ellipse", "ring", "polygon", "star", "line"][index])
                })
    }

    SceneEffectMaskTune {
        Kirigami.FormData.label: App.tr("Mask:")
        Layout.fillWidth: true
        panel: panel
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: panel.source !== ""
        wrapMode: Text.Wrap
        text: App.tr("The canvas shows the mask over the layer: drag the handles of a shape to size it and the dot past its edge to soften it.")
    }
}
