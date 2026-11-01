pragma Singleton
import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    function round(p) {
        return p.map(v => Math.round(v * 10) / 10);
    }

    function walls(w, c) {
        const loc = p => round(w.swarm.local(p));
        const dist = (a, b) => Math.round(Math.hypot(a[0] - b[0], a[1] - b[1]) * 10) / 10;
        switch (c.kind) {
        case "plane":
            return [{
                    "at": c.point,
                    "tip": App.tr("Drag to move the plane"),
                    "act": to => w.patch(c, v => v.point = loc(to))
                }, {
                    "at": c.tip,
                    "tip": App.tr("Drag to turn the side particles stay on"),
                    "act": to => w.patch(c, v => {
                                const a = loc(c.point), b = loc(to);
                                v.normal = round([b[0] - a[0], b[1] - a[1]]);
                            })
                }];
        case "circle":
            return [{
                    "at": c.center,
                    "act": to => w.patch(c, v => v.center = loc(to))
                }, {
                    "at": c.edge,
                    "tip": App.tr("Drag to change the radius"),
                    "act": to => w.patch(c, v => v.radius = Math.max(0.1, dist(loc(to), loc(c.center))))
                }];
        case "box":
            return [["from", 0], ["to", 2]].map(pair => ({
                            "at": c.corners[pair[1]],
                            "act": to => w.patch(c, v => v[pair[0]] = loc(to))
                        }));
        case "segment":
            return ["from", "to"].map(key => ({
                            "at": c[key],
                            "act": to => w.patch(c, v => v[key] = loc(to))
                        }));
        default:
            return [];
        }
    }

    function spot(k, e, ref, to, write) {
        if (typeof ref === "string" && /^p\d$/.test(ref))
            k.swarm.edit("point", {
                    "point": Number(ref.slice(1)),
                    "at": k.swarm.local(to)
                });
        else if (Array.isArray(ref))
            k.patch(e, v => write(v, round(k.swarm.local(to))));
    }

    function keeps(k, e) {
        if (e.kind === "keep_distance")
            return [{
                    "at": e.at,
                    "label": App.tr("target"),
                    "act": to => spot(k, e, e.target, to, (v, p) => v.target = p)
                }, {
                    "at": e.ring,
                    "tip": App.tr("Drag to change the distance kept"),
                    "act": to => k.patch(e, v => v.distance = Math.round(Math.hypot(to[0] - e.at[0], to[1] - e.at[1]) / Math.max(1e-6, e.radius / Math.max(1e-6, e.distance)) * 10) / 10)
                }];
        return [{
                "at": e.from,
                "label": App.tr("from"),
                "act": to => spot(k, e, e.from_ref, to, (v, p) => v.from = p)
            }, {
                "at": e.to,
                "label": App.tr("to"),
                "act": to => spot(k, e, e.to_ref, to, (v, p) => v.to = p)
            }];
    }
}
