import { state, setup, need, dirty, touched, guard, owner } from "wprs:core";
import { own } from "wprs:layers";

const LIMIT = 65536;
const kept = { package: { ...setup.storage?.package }, output: { ...setup.storage?.output } };
const changed = new Set();
const scope = (s = "package") => {
    if (s !== "package" && s !== "output") throw new TypeError("storage takes the scope package or output");
    return s;
};
const bytes = (text) => {
    let n = 0;
    for (let i = 0; i < text.length; i++) n += text.charCodeAt(i) < 0x80 ? 1 : 3;
    return n;
};
const save = (at, next) => {
    if (bytes(JSON.stringify(next)) > LIMIT) throw new RangeError("storage keeps at most 64 KiB in each scope");
    kept[at] = next;
    changed.add(at);
    dirty.add("storage");
};

export const storage = Object.freeze({
    get(key, s) {
        need("storage");
        const v = kept[scope(s)][String(key)];
        return v === undefined ? undefined : JSON.parse(JSON.stringify(v));
    },
    set(key, value, s) {
        need("storage");
        const text = JSON.stringify(value);
        if (text === undefined) throw new TypeError("storage keeps only JSON values");
        save(scope(s), { ...kept[scope(s)], [String(key)]: JSON.parse(text) });
    },
    delete(key, s) {
        need("storage");
        const { [String(key)]: gone, ...rest } = kept[scope(s)];
        if (gone !== undefined) save(scope(s), rest);
    },
    clear(s) {
        need("storage");
        if (Object.keys(kept[scope(s)]).length) save(scope(s), {});
    },
    keys: (s) => (need("storage"), Object.keys(kept[scope(s)])),
});

export const takeStorage = () => {
    if (!changed.size) return null;
    const out = Object.fromEntries([...changed].map((at) => [at, kept[at]]));
    changed.clear();
    return out;
};

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
