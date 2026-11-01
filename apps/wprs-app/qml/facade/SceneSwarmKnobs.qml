pragma Singleton

import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    function pointer(system, rest) {
        return "/scene/particles/" + system.replace(/~/g, "~0").replace(/\//g, "~1") + rest;
    }

    function back(e, loc) {
        const m = e.inverse;
        if (!m)
            return loc;
        return p => [Math.round((m[0] * p[0] + m[2] * p[1] + m[4]) * 10) / 10, Math.round((m[1] * p[0] + m[3] * p[1] + m[5]) * 10) / 10];
    }

    function spot(shape, ref, to, write) {
        if (typeof ref === "string" && /^p\d$/.test(ref))
            shape.swarm.edit("point", {
                "point": Number(ref.slice(1)),
                "at": shape.swarm.local(to)
            });
        else if (Array.isArray(ref))
            shape.patch(s => write(s, shape.swarm.local(to)));
    }

    function of(shape) {
        const e = shape.emitter, w = shape.swarm, loc = p => w.local(p);
        const put = back(e, loc), anchored = typeof e.anchor === "string";
        const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1]);
        switch (e.kind) {
        case "point":
            return [
                {
                    "at": e.at,
                    "tip": App.tr("Drag to move the emitter"),
                    "act": to => shape.set({
                            "point": loc(to)
                        })
                }
            ];
        case "line":
            return [
                {
                    "at": e.from,
                    "act": to => shape.set({
                            "line": [loc(to), loc(e.to)]
                        })
                },
                {
                    "at": e.to,
                    "act": to => shape.set({
                            "line": [loc(e.from), loc(to)]
                        })
                }
            ];
        case "box":
            return [0, 2].map(k => ({
                        "at": e.corners[k],
                        "tip": anchored ? App.tr("Drag to resize the box around its control point") : "",
                        "act": to => shape.patch(s => {
                                const key = k === 0 ? "from" : "to", i = k === 0 ? 0 : 1;
                                if (Array.isArray(s))
                                    s[i] = put(to);
                                else
                                    s[key] = put(to);
                            })
                    }));
        case "circle":
            return [
                {
                    "at": e.center,
                    "tip": anchored ? App.tr("Drag to move the circle away from its control point") : "",
                    "act": to => shape.patch(s => s.center = put(to))
                },
                {
                    "at": e.edge,
                    "tip": App.tr("Drag to change the radius"),
                    "act": to => shape.patch(s => s.radius = Math.round(dist(put(to), e.local) * 10) / 10)
                }
            ];
        case "around":
            if (!e.center)
                return [];
            return [
                {
                    "at": e.center,
                    "label": App.tr("center"),
                    "act": to => spot(shape, e.at, to, (s, p) => s.at = p)
                },
                {
                    "at": e.ends[0],
                    "tip": App.tr("Drag to change the radius"),
                    "act": to => shape.patch(s => s.radius = Math.round(dist(to, e.center) / Math.max(1e-6, dist(e.ends[0], e.center) / Math.max(1e-6, s.radius)) * 10) / 10)
                },
                {
                    "at": e.ends[1],
                    "tip": App.tr("Drag to change the arc"),
                    "act": to => shape.patch(s => s.arc = Math.round(((Math.atan2(to[1] - e.center[1], to[0] - e.center[0]) * 180 / Math.PI - e.start) % 360 + 360) % 360) || 360)
                }
            ];
        case "between":
            if (!e.from || !e.to)
                return [];
            return [
                {
                    "at": e.from,
                    "label": App.tr("from"),
                    "act": to => spot(shape, e.from_ref, to, (s, p) => s.from = p)
                },
                {
                    "at": e.to,
                    "label": App.tr("to"),
                    "act": to => spot(shape, e.to_ref, to, (s, p) => s.to = p)
                },
                {
                    "at": e.bulge,
                    "tip": App.tr("Drag to bow the line"),
                    "act": to => shape.patch(s => {
                            const d = [e.to[0] - e.from[0], e.to[1] - e.from[1]], l2 = Math.max(1e-6, d[0] * d[0] + d[1] * d[1]);
                            const m = [(e.from[0] + e.to[0]) / 2, (e.from[1] + e.to[1]) / 2];
                            s.arc = {
                                "amount": Math.round(((to[0] - m[0]) * -d[1] + (to[1] - m[1]) * d[0]) / l2 * 100) / 100,
                                "side": s.arc?.side ?? 1
                            };
                        })
                }
            ].concat(e.taper ? [
                {
                    "at": e.taper.at,
                    "tip": App.tr("Drag along the line to move the widest point"),
                    "act": to => shape.patch(s => {
                            const d = [e.to[0] - e.from[0], e.to[1] - e.from[1]], l2 = Math.max(1e-6, d[0] * d[0] + d[1] * d[1]);
                            const u = ((to[0] - e.from[0]) * d[0] + (to[1] - e.from[1]) * d[1]) / l2 / Math.max(1e-6, e.span);
                            s.taper = Object.assign(s.taper ?? {}, {
                                "position": Math.round(Math.min(1, Math.max(0, u)) * 100) / 100
                            });
                        })
                }
            ] : []);
        default:
            return [];
        }
    }
}
