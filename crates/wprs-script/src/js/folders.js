import { setup, need, touched, guard, owner } from "wprs:core";
import { own } from "wprs:layers";

const listing = {};
const handlers = {};
const EVENTS = ["added", "removed", "changed"];

export function folder(prop) {
    need("folders");
    prop = String(prop);
    if (!setup.folders?.includes(prop)) throw new TypeError(`${prop} is not a folder property that lists its files`);
    const files = () => listing[prop] ?? [];
    return Object.freeze({
        get files() {
            return files().map((f) => f[0]);
        },
        name: (id) => files().find((f) => f[0] === id)?.[2] ?? null,
        on(event, fn) {
            if (!EVENTS.includes(event) || typeof fn !== "function") throw new TypeError(`on takes ${EVENTS.join(", ")} and a function`);
            ((handlers[prop] ??= {})[event] ??= []).push([owner(), fn]);
        },
        show(layer, id) {
            layer = String(layer);
            if (!setup.layers[layer] && !own.spawned.has(layer.split(".")[0])) throw new TypeError(`${layer} is not a layer`);
            if (id !== null && !files().some((f) => f[0] === id)) throw new RangeError(`${id} is not a file of ${prop}`);
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
        const [ids, keys] = [new Set(before.map((f) => f[0])), new Map(before.map((f) => [f[1], f[0]]))];
        const fresh = now.filter((f) => !ids.has(f[0]));
        const kept = new Set(now.map((f) => f[0]));
        const changed = fresh.filter((f) => keys.has(f[1])).map((f) => [keys.get(f[1]), f[0]]);
        const moved = new Set(changed.flat());
        const found = { added: fresh.filter((f) => !moved.has(f[0])).map((f) => f[0]), removed: before.filter((f) => !kept.has(f[0]) && !moved.has(f[0])).map((f) => f[0]), changed };
        for (const event of EVENTS) {
            if (!found[event].length) continue;
            const e = Object.freeze({ prop, files: found[event] });
            (handlers[prop]?.[event] ?? []).forEach(([mod, fn]) => guard(mod, fn, e));
        }
    }
}
