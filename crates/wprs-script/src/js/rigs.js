import { state, need, dirty } from "wprs:core";
import { numbers, number } from "wprs:values";
import { info, watch } from "wprs:layers";

export const rigState = {};
const DEG = 180 / Math.PI;
const mul = (p, c) => [p[0] * c[0] + p[2] * c[1], p[1] * c[0] + p[3] * c[1], p[0] * c[2] + p[2] * c[3], p[1] * c[2] + p[3] * c[3], p[0] * c[4] + p[2] * c[5] + p[4], p[1] * c[4] + p[3] * c[5] + p[5]];
const inv = ([a, b, c, d, e, f]) => {
    const det = a * d - b * c || 1;
    const [x, y, z, w] = [d / det, -b / det, -c / det, a / det];
    return [x, y, z, w, -(x * e + z * f), -(y * e + w * f)];
};
const turn = (m) => Math.atan2(m[1], m[0]) * DEG;
const wrap = (a) => ((((a + 180) % 360) + 360) % 360) - 180;
const stretch = (m) => {
    const sx = Math.hypot(m[0], m[1]) || 1;
    return [sx, (m[0] * m[3] - m[1] * m[2]) / sx];
};

function rigOf(id) {
    const r = info(id)?.rig;
    if (!r) throw new TypeError(`${id} is not a mesh layer`);
    return r;
}

function frames(id) {
    watch(id);
    watch(`${id}#bones`);
    return state.frame.reads[id]?.bones ?? [];
}

const own = (id) => (rigState[id] ??= { bones: {}, shapes: {} });
const index = (r, id, name) => {
    const at = r.bones.findIndex((b) => b.name === String(name));
    if (at < 0) throw new TypeError(`${id} has no bone ${name}`);
    return at;
};

function local(id, r, at) {
    const f = frames(id);
    if (!f[at]) return null;
    const p = r.bones[at].parent;
    const base = p === null ? [1, 0, 0, 1, r.size[0] / 2, r.size[1] / 2] : f[p];
    return base ? mul(inv(base), f[at]) : null;
}

function bone(id, name) {
    need("animations");
    const r = rigOf(id);
    const at = index(r, id, name);
    const rest = r.bones[at];
    const set = (fields) => {
        own(id).bones[rest.name] = { ...own(id).bones[rest.name], ...fields };
        dirty.add("rigs");
    };
    const given = (key, world) => {
        const b = own(id).bones[rest.name];
        return b && Boolean(b.world) === world ? b[key] : undefined;
    };
    const offsets = () => {
        const l = local(id, r, at);
        if (!l) return { rotation: 0, position: [0, 0], scale: [1, 1] };
        const s = stretch(l);
        return { rotation: wrap(turn(l) - rest.rotation), position: [l[4] - rest.position[0], l[5] - rest.position[1]], scale: [s[0] / rest.scale[0], s[1] / rest.scale[1]] };
    };
    const world = () => frames(id)[at] ?? [1, 0, 0, 1, 0, 0];
    return Object.freeze({
        name: rest.name,
        parent: rest.parent === null ? null : r.bones[rest.parent].name,
        get rotation() {
            return given("rotation", false) ?? offsets().rotation;
        },
        set rotation(v) {
            set({ rotation: number("rotation")(v), world: false, position: given("position", false) });
        },
        get position() {
            return given("position", false) ?? offsets().position;
        },
        set position(v) {
            set({ position: numbers(v, 2, "position"), world: false, rotation: given("rotation", false) });
        },
        get scale() {
            return own(id).bones[rest.name]?.scale ?? offsets().scale;
        },
        set scale(v) {
            set({ scale: typeof v === "number" ? [number("scale")(v), v] : numbers(v, 2, "scale") });
        },
        get worldPosition() {
            return given("position", true) ?? [world()[4], world()[5]];
        },
        set worldPosition(v) {
            set({ position: numbers(v, 2, "worldPosition"), world: true, rotation: given("rotation", true) });
        },
        get worldRotation() {
            return given("rotation", true) ?? turn(world());
        },
        set worldRotation(v) {
            set({ rotation: number("worldRotation")(v), world: true, position: given("position", true) });
        },
        reset() {
            delete own(id).bones[rest.name];
            dirty.add("rigs");
        },
    });
}

function hit(id, x, y) {
    need("animations");
    const r = rigOf(id);
    const f = frames(id);
    const names = r.hit.length ? r.hit : r.bones.map((b) => b.name);
    let best = [null, Infinity];
    for (const name of names) {
        const at = r.bones.findIndex((b) => b.name === name);
        if (!f[at]) continue;
        const head = [f[at][4], f[at][5]];
        const child = r.bones.findIndex((b) => b.parent === at);
        const tail = f[child] ? [f[child][4], f[child][5]] : head;
        const [dx, dy] = [tail[0] - head[0], tail[1] - head[1]];
        const k = Math.max(0, Math.min(1, ((x - head[0]) * dx + (y - head[1]) * dy) / (dx * dx + dy * dy || 1)));
        const d = Math.hypot(x - head[0] - k * dx, y - head[1] - k * dy);
        if (d < best[1]) best = [name, d];
    }
    return best[0];
}

function attachment(id, name) {
    need("animations");
    const r = rigOf(id);
    const at = index(r, id, name);
    const read = state.frame.reads[id];
    const f = frames(id)[at] ?? [1, 0, 0, 1, 0, 0];
    const [ox, oy] = read?.origin ?? [0.5, 0.5];
    const corner = [1, 0, 0, 1, -ox * r.size[0], -oy * r.size[1]];
    const matrix = mul(mul(read?.world ?? [1, 0, 0, 1, 0, 0], corner), f);
    return Object.freeze({ matrix, origin: [matrix[4], matrix[5]], angle: turn(matrix), uv: [f[4] / r.size[0], f[5] / r.size[1]] });
}

export function rigged(h, id) {
    Object.defineProperty(h, "bones", { enumerable: true, get: () => rigOf(id).bones.map((b) => b.name) });
    h.bone = (name) => bone(id, name);
    h.hit = (x, y) => hit(id, number("x")(x), number("y")(y));
    h.attachment = (name) => attachment(id, name);
    h.shape = (name, weight) => {
        need("animations");
        if (!rigOf(id).shapes.includes(String(name)) && !(String(name) in (info(id)?.initial?.expressions ?? {}))) throw new TypeError(`${id} has no blend shape ${name}`);
        if (weight === undefined) return own(id).shapes[String(name)] ?? null;
        own(id).shapes[String(name)] = Math.min(1, Math.max(-1, number("weight")(weight)));
        dirty.add("rigs");
    };
    h.impulse = (name, v) => {
        need("animations");
        index(rigOf(id), id, name);
        ((state.out.rigs[id] ??= []).push([String(name), numbers(v, 2, "impulse")]));
    };
}

export const rigsOut = (out) => Object.fromEntries([...new Set([...Object.keys(rigState), ...Object.keys(out.rigs)])].map((id) => [id, { ...(rigState[id] ?? {}), impulses: out.rigs[id] ?? [] }]));
