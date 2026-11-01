import { setup, need, dirty } from "wprs:core";

const LIMIT = 65536;
const KEPT = 4;
const kept = { package: { ...setup.storage?.package }, output: { ...setup.storage?.output } };
const ops = { package: new Map(), output: new Map() };
let [seq, unsure] = [0, []];
const scope = (s = "package") => {
    if (s !== "package" && s !== "output") throw new TypeError("storage takes the scope package or output");
    return s;
};
const bytes = (text) => {
    let n = 0;
    for (let i = 0; i < text.length; i++) n += text.charCodeAt(i) < 0x80 ? 1 : 3;
    return n;
};
const put = (map, key, value) => {
    const { [key]: _, ...rest } = map;
    return value === undefined ? rest : { ...rest, [key]: value };
};
const save = (at, changes) => {
    let next = kept[at];
    for (const [key, value] of changes) next = put(next, key, value);
    if (bytes(JSON.stringify(next)) > LIMIT) throw new RangeError("storage keeps at most 64 KiB in each scope");
    kept[at] = next;
    for (const [key, value] of changes) ops[at].set(key, value);
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
        save(scope(s), [[String(key), JSON.parse(text)]]);
    },
    delete(key, s) {
        need("storage");
        if (kept[scope(s)][String(key)] !== undefined) save(scope(s), [[String(key), undefined]]);
    },
    clear(s) {
        need("storage");
        const keys = Object.keys(kept[scope(s)]);
        if (keys.length) save(scope(s), keys.map((key) => [key, undefined]));
    },
    keys: (s) => (need("storage"), Object.keys(kept[scope(s)])),
});

export const takeStorage = () => {
    if (!ops.package.size && !ops.output.size) return null;
    const out = { seq: ++seq, set: { package: {}, output: {} }, gone: { package: [], output: [] } };
    for (const at of ["package", "output"]) {
        for (const [key, value] of ops[at]) {
            if (value === undefined) out.gone[at].push(key);
            else out.set[at][key] = value;
            unsure.push([seq, at, key, value]);
        }
        ops[at].clear();
    }
    unsure = unsure.filter(([n]) => n > seq - KEPT);
    return out;
};

export function storageFrom(text) {
    if (!text) return;
    const got = JSON.parse(text);
    unsure = unsure.filter(([n]) => n > got.seq);
    for (const at of ["package", "output"]) kept[at] = { ...got[at] };
    for (const [, at, key, value] of unsure) kept[at] = put(kept[at], key, value);
}
