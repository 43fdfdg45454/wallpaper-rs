import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: strip

    required property var page
    readonly property var mesh: page.mesh
    property var shown: null
    property var chosen: null
    readonly property real length: Math.max(0.1, shown?.length ?? 2)
    readonly property string repeat: shown?.loop ?? "repeat"
    readonly property real now: local(page.clock)
    readonly property var picked: chosen ? (rows.find(r => same(r, chosen.row))?.keys ?? []).find(k => Math.abs(k.t - chosen.t) < 1e-3) ?? null : null
    readonly property var rows: compose(shown, mesh.bone)

    function local(t) {
        if (repeat === "once")
            return Math.min(t, length);
        const u = t % (repeat === "pingpong" ? 2 * length : length);
        return u > length ? 2 * length - u : u;
    }

    function same(a, b) {
        return a.kind === b.kind && a.name === b.name && a.channel === b.channel;
    }

    function compose(s, bone) {
        const listed = (s?.rows ?? []).filter(r => r.kind !== "shape");
        const own = ["rotation", "position", "scale"].map(c => ({
                    "kind": "bone",
                    "name": bone,
                    "channel": c,
                    "keys": []
                })).filter(r => bone !== "" && !listed.some(l => same(l, r)));
        const shapes = (s?.shapes ?? []).map(name => (s.rows.find(r => r.kind === "shape" && r.name === name)) ?? {
                "kind": "shape",
                "name": name,
                "channel": "",
                "keys": []
            });
        return listed.concat(own).sort((a, b) => a.kind === b.kind && a.kind === "bone" ? a.name.localeCompare(b.name) : 0).concat(shapes);
    }

    function load() {
        shown = mesh.clip !== "" ? mesh.ask(mesh.layer, {
            "keys": mesh.clip
        }) : null;
    }

    function seek(t, playing) {
        page.clock = Math.max(0, t);
        page.editor.seek(page.clock, playing);
    }

    function key(row, args) {
        const named = row.kind === "bone" ? {
            "bone": row.name
        } : row.kind === "shape" ? {
            "shape": row.name
        } : {
            "order": row.name
        };
        page.run("key", Object.assign({
            "layer": mesh.layer,
            "clip": mesh.clip
        }, named, args));
    }

    function held(row, k) {
        const field = row.kind === "bone" ? row.channel : row.kind === "shape" ? "weight" : "value";
        const out = {};
        out[field] = k.value;
        return out;
    }

    spacing: 0
    onShownChanged: if (chosen && !picked)
        chosen = null

    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            strip.load();
        }

        target: strip.page.editor
    }

    Connections {
        function onClipChanged() {
            strip.chosen = null;
            strip.load();
        }

        target: strip.mesh
    }

    Timer {
        interval: 50
        repeat: true
        running: strip.page.editor.playing && strip.visible
        onTriggered: strip.page.clock += interval / 1000
    }

    SceneClipBar {
        Layout.fillWidth: true
        line: strip
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: strip.rows
            clip: true

            delegate: SceneClipRow {
                width: ListView.view.width
                line: strip
            }
        }
    }
}
