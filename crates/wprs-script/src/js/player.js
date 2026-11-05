import { setup, state, records, guard, newer, owner } from "wprs:core";

const NAMES = ["title", "artist", "album", "album_artist", "subtitle", "genres", "kind"];
const SHADES = ["primary", "secondary", "tertiary", "text", "contrast"];
const EMPTY = Object.freeze({ enabled: false, state: "stopped", track: 0, cover: 0, has_cover: false, colors: null, position: 0, duration: 0, rate: 1, at: 0, ...Object.fromEntries(NAMES.map((n) => [n, ""])) });
let [now, started] = [EMPTY, false];

const hears = (path) => !setup.checked || (setup.media ?? []).includes(path);
const look = () => {
    newer("media");
    const path = owner().path;
    if (!hears(path)) throw new TypeError(`${path} uses media without declaring it in scene.json`);
};
const reach = (s, at) => {
    const moved = s.state === "playing" ? s.position + (at - s.at) * s.rate : s.position;
    return s.duration > 0 ? Math.min(Math.max(moved, 0), s.duration) : Math.max(moved, 0);
};
const shade = (c) => Object.freeze({ r: c[0], g: c[1], b: c[2], a: c[3] });
const shades = (list) => (Array.isArray(list) ? Object.freeze(Object.fromEntries(SHADES.map((k, i) => [k, shade(list[i])]))) : null);
const views = {
    mediaStatus: () => ({ enabled: now.enabled }),
    mediaPlayback: () => ({ state: now.state }),
    mediaProperties: () => Object.fromEntries(NAMES.map((n) => [n, now[n]])),
    mediaThumbnail: () => ({ has_cover: now.has_cover, colors: shades(now.colors) }),
    mediaTimeline: () => ({ position: reach(now, state.frame.time), duration: now.duration, rate: now.rate }),
};
const changes = {
    mediaStatus: (a, b) => a.enabled !== b.enabled || !started,
    mediaPlayback: (a, b) => a.state !== b.state,
    mediaProperties: (a, b) => a.track !== b.track || NAMES.some((n) => a[n] !== b[n]),
    mediaThumbnail: (a, b) => a.cover !== b.cover || JSON.stringify(a.colors) !== JSON.stringify(b.colors),
    mediaTimeline: (a, b) => a.track !== b.track || a.duration !== b.duration || a.rate !== b.rate || Math.abs(b.position - reach(a, b.at)) > 0.5,
};
const whole = () => {
    const timeline = views.mediaTimeline();
    const progress = timeline.duration > 0 ? Math.min(1, timeline.position / timeline.duration) : 0;
    return Object.freeze({ ...views.mediaStatus(), ...views.mediaPlayback(), playing: now.state === "playing", ...views.mediaProperties(), ...views.mediaThumbnail(), ...timeline, progress });
};
const KEYS = Object.keys(whole());

export const media = Object.freeze(
    Object.defineProperties(
        {
            snapshot() {
                look();
                return whole();
            },
        },
        Object.fromEntries(KEYS.map((k) => [k, { enumerable: true, get: () => (look(), whole()[k]) }])),
    ),
);

export function mediaEvents(text) {
    if (!text) return;
    const before = now;
    now = Object.freeze({ ...EMPTY, ...JSON.parse(text) });
    for (const name of Object.keys(changes)) {
        if (!changes[name](before, now)) continue;
        const event = Object.freeze(views[name]());
        for (const mod of records.values()) if (!mod.first && hears(mod.path) && typeof mod.ns?.[name] === "function") guard(mod, mod.ns[name], event);
    }
    started = true;
}
