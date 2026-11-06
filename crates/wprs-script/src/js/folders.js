import { setup, need, touched, guard, owner } from "wprs:core";
import { own, info } from "wprs:layers";

const listing = {};
const handlers = {};
const EVENTS = ["added", "removed", "changed"];

export function folder(prop, options) {
    need("folders");
    prop = String(prop);
    if (!setup.folders?.includes(prop)) throw new TypeError(`${prop} is not a folder property that lists its files`);
    const videos = options?.videos === true;
    const files = () => (listing[prop] ?? []).filter((f) => videos || f[3] !== "video");
    const found = (id) => files().find((f) => f[0] === id);
    return Object.freeze({
        get files() {
            return files().map((f) => f[0]);
        },
        name: (id) => found(id)?.[2] ?? null,
        kind: (id) => (found(id) ? (found(id)[3] === "video" ? "video" : "image") : null),
        on(event, fn) {
            if (!EVENTS.includes(event) || typeof fn !== "function") throw new TypeError(`on takes ${EVENTS.join(", ")} and a function`);
            ((handlers[prop] ??= {})[event] ??= []).push([owner(), fn, videos]);
        },
        show(layer, id) {
            layer = String(layer);
            const type = info(layer)?.type;
            if (!type) throw new TypeError(`${layer} is not a layer`);
            const file = id === null ? null : found(id);
            if (id !== null && !file) throw new RangeError(`${id} is not a file of ${prop}`);
            if (file?.[3] === "video" && type !== "video") throw new TypeError(`${layer} is no video layer and cannot show a video`);
            const set = (own.overrides[layer] ??= {});
            if (id === null) delete set.file;
            else set.file = `wpak-user://${prop}/${id}`;
            touched.add(layer);
        },
    });
}

export function folderEvents(text) {
    if (!text) return;
    for (const [prop, now] of Object.entries(JSON.parse(text))) {
        const before = listing[prop] ?? [];
        listing[prop] = now;
        const kinds = new Map(before.concat(now).map((f) => [f[0], f[3]]));
        const [ids, keys] = [new Set(before.map((f) => f[0])), new Map(before.map((f) => [f[1], f[0]]))];
        const fresh = now.filter((f) => !ids.has(f[0]));
        const kept = new Set(now.map((f) => f[0]));
        const changed = fresh.filter((f) => keys.has(f[1])).map((f) => [keys.get(f[1]), f[0]]);
        const moved = new Set(changed.flat());
        const found = { added: fresh.filter((f) => !moved.has(f[0])).map((f) => f[0]), removed: before.filter((f) => !kept.has(f[0]) && !moved.has(f[0])).map((f) => f[0]), changed };
        for (const event of EVENTS) {
            for (const [mod, fn, videos] of handlers[prop]?.[event] ?? []) {
                const seen = (id) => videos || kinds.get(id) !== "video";
                const files = found[event].filter((f) => seen(Array.isArray(f) ? f[1] : f));
                if (files.length) guard(mod, fn, Object.freeze({ prop, files }));
            }
        }
    }
}
