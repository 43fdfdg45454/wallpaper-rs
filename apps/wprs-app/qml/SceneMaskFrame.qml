import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: frame

    required property var overlay
    required property var shown
    property bool outlined: false
    readonly property var g: shown ?? {
        "offset": [0, 0],
        "position": [0, 0],
        "size": [1, 1],
        "scale": [1, 1],
        "rotation": 0
    }
    readonly property var stretch: [width / Math.max(1, g.size[0]), height / Math.max(1, g.size[1])]
    readonly property bool shaped: !outlined && overlay.kind !== ""
    readonly property string picture: {
        const found = (overlay.page.view.assets ?? []).find(a => a.id === overlay.texture);
        return found ? overlay.page.folder + "/" + found.path : "";
    }

    function local(item, point) {
        const p = item.mapToItem(frame, point.x, point.y);
        return [(p.x - width / 2) / stretch[0], (p.y - height / 2) / stretch[1]];
    }

    visible: shown !== null
    x: (g.offset[0] + g.position[0]) * overlay.ratio - width / 2
    y: (g.offset[1] + g.position[1]) * overlay.ratio - height / 2
    width: Math.max(1, Math.abs(g.size[0] * g.scale[0]) * overlay.ratio)
    height: Math.max(1, Math.abs(g.size[1] * g.scale[1]) * overlay.ratio)
    rotation: g.rotation
    z: 3

    Rectangle {
        anchors.fill: parent
        visible: frame.outlined
        color: Qt.rgba(Kirigami.Theme.neutralTextColor.r, Kirigami.Theme.neutralTextColor.g, Kirigami.Theme.neutralTextColor.b, 0.15)
        border.color: Kirigami.Theme.neutralTextColor
        border.width: 2
    }

    Image {
        anchors.fill: parent
        visible: !frame.outlined && frame.overlay.texture !== ""
        opacity: 0.45
        fillMode: Image.Stretch
        asynchronous: true
        source: visible && flat.flat !== "" ? "file://" + flat.flat : ""
    }

    Packer {
        id: flat

        Component.onCompleted: flatten(frame.picture)
    }

    onPictureChanged: flat.flatten(picture)

    SceneMaskShape {
        visible: frame.shaped
        kind: frame.overlay.kind
        spec: frame.overlay.spec
        box: frame.overlay.box
        stretch: frame.stretch
    }

    SceneMaskShape {
        visible: frame.shaped && frame.overlay.kind === "ring"
        kind: "ellipse"
        spec: ({})
        box: frame.overlay.box.map(v => Math.max(0, v - 2 * (frame.overlay.spec.width ?? 0)))
        stretch: frame.stretch
        dashed: true
    }

    Repeater {
        model: frame.shaped && (frame.overlay.mask.feather ?? 0) > 0 ? [-1, 1] : []

        delegate: SceneMaskShape {
            required property int modelData

            kind: frame.overlay.kind
            spec: frame.overlay.spec
            box: frame.overlay.box
            stretch: frame.stretch
            grow: modelData * frame.overlay.mask.feather
            dashed: true
        }
    }

    SceneMaskKnobs {
        anchors.fill: parent
        visible: frame.shaped
        frame: frame
    }
}
