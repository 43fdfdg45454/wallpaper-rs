import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: poser

    required property var host
    readonly property var mesh: host.mesh
    readonly property var page: host.gizmo.page
    property var shown: null
    readonly property var picked: (shown?.bones ?? []).find(b => b.name === mesh.bone) ?? null
    readonly property real reach: Kirigami.Units.gridUnit * 2.5
    property var start: null

    function load() {
        if (!visible || page.editor.playing)
            return;
        shown = mesh.ask(host.layerId, {
            "pose": Object.assign({
                "clip": mesh.clip,
                "time": mesh.time,
                "bone": mesh.bone
            }, mesh.pending(mesh.bone))
        });
        lines.requestPaint();
    }

    function held() {
        const p = mesh.pose && mesh.pose.bone === mesh.bone ? mesh.pose : null;
        return {
            "bone": mesh.bone,
            "rotation": p ? p.rotation : picked.rotation,
            "position": p ? p.position : picked.position,
            "scale": p ? p.scale : picked.scale,
            "changed": p?.changed ?? []
        };
    }

    function point(at) {
        return Qt.point(at[0] * width, at[1] * height);
    }

    function grab(where) {
        start = Object.assign({
            "where": where,
            "head": point(picked.at)
        }, held());
    }

    function drag(kind, pos) {
        if (!start)
            return;
        const h = start.head, w = start.where, s = shown.size, f = picked.frame;
        const pose = Object.assign({}, start);
        const channel = kind === "move" ? "position" : kind === "turn" ? "rotation" : "scale";
        pose.changed = start.changed.includes(channel) ? start.changed : start.changed.concat([channel]);
        if (kind === "move") {
            const mx = (pos.x - w.x) / width * s[0], my = -(pos.y - w.y) / height * s[1];
            const det = f[0] * f[3] - f[1] * f[2] || 1;
            const lx = (f[3] * mx - f[1] * my) / det, ly = (-f[2] * mx + f[0] * my) / det;
            pose.position = [Math.round((start.position[0] + lx) * 10) / 10, Math.round((start.position[1] - ly) * 10) / 10];
        } else if (kind === "turn") {
            const turned = Math.atan2(pos.y - h.y, pos.x - h.x) - Math.atan2(w.y - h.y, w.x - h.x);
            pose.rotation = Math.round((start.rotation + turned * 180 / Math.PI) * 10) / 10;
        } else {
            const grown = Math.hypot(pos.x - h.x, pos.y - h.y) / Math.max(1, Math.hypot(w.x - h.x, w.y - h.y));
            pose.scale = start.scale.map(v => Math.round(v * grown * 1000) / 1000);
        }
        delete pose.where;
        delete pose.head;
        mesh.pose = pose;
    }

    onVisibleChanged: load()

    Connections {
        function onPoseChanged() {
            poser.load();
        }

        function onBoneChanged() {
            poser.load();
        }

        function onTimeChanged() {
            poser.load();
        }

        target: poser.mesh
    }

    Connections {
        function onViewChanged() {
            poser.load();
        }

        function onPlayingChanged() {
            poser.load();
        }

        target: poser.page.editor
    }

    Canvas {
        id: lines

        anchors.fill: parent
        visible: !poser.page.editor.playing
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            const bones = poser.shown?.bones ?? [];
            ctx.strokeStyle = Kirigami.Theme.highlightColor;
            ctx.lineWidth = 2;
            for (const b of bones) {
                const up = bones.find(u => u.name === (poser.host.list.find(r => r.name === b.name)?.parent));
                if (up) {
                    ctx.beginPath();
                    ctx.moveTo(up.at[0] * width, up.at[1] * height);
                    ctx.lineTo(b.at[0] * width, b.at[1] * height);
                    ctx.stroke();
                }
                ctx.fillStyle = b.name === poser.mesh.bone ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.highlightColor;
                ctx.beginPath();
                ctx.arc(b.at[0] * width, b.at[1] * height, 4, 0, 2 * Math.PI);
                ctx.fill();
            }
        }

        TapHandler {
            onTapped: point => {
                const bones = poser.shown?.bones ?? [];
                const near = bones.map(b => [b, Math.hypot(b.at[0] * poser.width - point.position.x, b.at[1] * poser.height - point.position.y)]).sort((a, b) => a[1] - b[1])[0];
                if (near && near[1] < Kirigami.Units.gridUnit)
                    poser.mesh.bone = near[0].name;
            }
        }
    }

    Repeater {
        model: poser.picked && !poser.page.editor.playing ? ["move", "turn", "grow"] : []

        delegate: ScenePoseKnob {
            handles: poser
        }
    }
}
