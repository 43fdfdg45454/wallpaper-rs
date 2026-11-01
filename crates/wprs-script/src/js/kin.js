import { state, setup, need, touched } from "wprs:core";
import { Mat3 } from "wprs:matrices";
import { own, info, watch } from "wprs:layers";

const DEPTH = 8;
let homes = null;
const home = (id) => {
    if (!homes) {
        homes = new Map();
        for (const [group, i] of Object.entries(setup.layers)) for (const child of i.initial?.layers ?? []) homes.set(String(child), group);
    }
    return homes.get(id) ?? null;
};
const explicit = (id) => {
    const set = own.overrides[id]?.parent;
    return set === undefined ? (info(id)?.initial?.parent ?? null) : set || null;
};
const parentOf = (id) => explicit(id) ?? home(id);
const ancestors = (id) => {
    const found = [];
    for (let at = id; at && found.length <= DEPTH; at = parentOf(at)) found.push(at);
    return found;
};
const worldOf = (id) => {
    watch(id);
    const [a, b, c, d, e, f] = state.frame.reads[id]?.world ?? [1, 0, 0, 1, 0, 0];
    return new Mat3([a, b, 0, c, d, 0, e, f, 1]);
};

export function family(h, id) {
    Object.defineProperties(h, {
        parent: { enumerable: true, get: () => (need("hierarchy"), parentOf(id)) },
        children: {
            enumerable: true,
            get() {
                need("hierarchy");
                const nested = (info(id)?.initial?.layers ?? []).map(String).filter((c) => !explicit(c));
                return [...nested, ...Object.keys(setup.layers).filter((k) => k !== id && explicit(k) === id)];
            },
        },
        world: { enumerable: true, get: () => (need("hierarchy"), worldOf(id)) },
    });
    h.setParent = (parent, keep = false) => {
        need("hierarchy");
        const p = parent === null || parent === undefined ? "" : String(parent);
        if (p && !setup.layers[p]) throw new TypeError(`${p} is not a layer of the scene`);
        const chain = p ? ancestors(p) : [];
        if (chain.includes(id)) throw new RangeError(`${id} cannot follow a layer that follows it`);
        if (chain.length >= DEPTH) throw new RangeError(`layers follow at most ${DEPTH} parents`);
        const set = (own.overrides[id] ??= {});
        if (keep) {
            const base = p ? worldOf(p) : home(id) ? worldOf(home(id)) : Mat3.translation(0, 0);
            const { translation, rotation, scale } = base.invert().multiply(worldOf(id)).decompose();
            Object.assign(set, { position: translation.toArray(), rotation, scale: scale.toArray(), skew: [0, 0] });
        }
        set.parent = p;
        touched.add(id);
    };
}
