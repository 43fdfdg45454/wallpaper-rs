import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: gizmo

    required property var owner
    required property var light
    readonly property real ratio: owner.ratio
    readonly property bool chosen: owner.selected === light.pointer
    readonly property var at: light.at

    function local(p) {
        const m = light.parent;
        return [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]].map(v => Math.round(v * 100) / 100);
    }

    function put(field, value) {
        owner.page.change(light.pointer + "/" + field, value, true);
    }

    function aim(to) {
        const a = local(at), b = local(to);
        put("transform/rotation", Math.round((Math.atan2(b[1] - a[1], b[0] - a[0]) * 180 / Math.PI - light.base) * 10) / 10);
    }

    function cone(to, inner) {
        const turn = Math.atan2(to[1] - at[1], to[0] - at[0]) * 180 / Math.PI - light.angle;
        const wide = Math.min(180, Math.round(Math.abs(((turn + 540) % 360) - 180) * 20) / 10);
        put("cone", inner ? [Math.min(wide, light.cone[1]), light.cone[1]] : [light.cone[0], Math.max(wide, light.cone[0])]);
    }

    function end(to, other, plus) {
        const mid = [(to[0] + other[0]) / 2, (to[1] + other[1]) / 2];
        const a = local(plus ? other : to), b = local(plus ? to : other);
        put("transform/position", local(mid));
        put("length", Math.max(1, Math.round(Math.hypot(to[0] - other[0], to[1] - other[1]))));
        put("transform/rotation", Math.round(Math.atan2(b[1] - a[1], b[0] - a[0]) * 1800 / Math.PI) / 10);
    }

    Rectangle {
        visible: gizmo.chosen || gizmo.light.type === "point"
        x: (gizmo.at[0] - gizmo.light.radius) * gizmo.ratio
        y: (gizmo.at[1] - gizmo.light.radius) * gizmo.ratio
        width: gizmo.light.radius * 2 * gizmo.ratio
        height: width
        radius: width / 2
        color: "transparent"
        border.color: Kirigami.Theme.neutralTextColor
        opacity: gizmo.chosen ? 0.9 : 0.35
    }

    Rectangle {
        visible: gizmo.chosen && gizmo.owner.page.lightBoxes === true
        x: gizmo.light.box[0] * gizmo.ratio
        y: gizmo.light.box[1] * gizmo.ratio
        width: gizmo.light.box[2] * gizmo.ratio
        height: gizmo.light.box[3] * gizmo.ratio
        color: "transparent"
        border.color: Kirigami.Theme.negativeTextColor
        border.width: 2
    }

    SceneLightCone {
        anchors.fill: parent
        gizmo: gizmo
    }

    SceneLightLines {
        anchors.fill: parent
        visible: gizmo.chosen
        ratio: gizmo.ratio
        segments: (gizmo.light.outer ?? []).concat(gizmo.light.inner ?? []).map(p => [gizmo.at, p]).concat(gizmo.light.aim ? [[gizmo.at, gizmo.light.aim]] : []).concat(gizmo.light.ends ? [gizmo.light.ends] : []).concat([[gizmo.at, gizmo.light.z_knob]])
    }

    SceneKnob {
        at: gizmo.at
        ratio: gizmo.ratio
        label: gizmo.light.id
        tint: gizmo.light.muted ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.neutralTextColor
        tip: App.tr("Drag to move the light")
        onMoved: (to, done) => {
            gizmo.owner.page.editor.select(gizmo.light.pointer);
            gizmo.put("transform/position", gizmo.local(to));
        }
    }

    SceneLightKnobs {
        gizmo: gizmo
    }
}
