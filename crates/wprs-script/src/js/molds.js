import { state, setup, need, dirty } from "wprs:core";

const FORMATS = { xy: 2, xyuv: 4, xyrgba: 6, xyuvrgba: 8 };
const LIMIT = 1 << 20;
const pools = setup.pools ?? {};
const molds = {};
const sent = new Set();
const budget = { bytes: 0, count: 0 };

function shape(o, format = "xyuv") {
    if (o === null || typeof o !== "object") throw new TypeError("a shape takes { vertices, indices, format }");
    const f = o.format ?? format;
    const stride = FORMATS[f];
    if (!stride) throw new TypeError(`format takes ${Object.keys(FORMATS).join(", ")}`);
    const vertices = new Float32Array(o.vertices ?? []);
    if (!vertices.length || vertices.length % stride) throw new RangeError(`${f} vertices hold ${stride} numbers each`);
    const n = vertices.length / stride;
    const indices = o.indices === undefined ? Uint32Array.from({ length: n - (n % 3) }, (_, i) => i) : new Uint32Array(o.indices);
    if (indices.length % 3) throw new RangeError("indices come in threes");
    if (indices.length > 6 * n) throw new RangeError("a shape has at most six indices a vertex");
    if (indices.some((i) => i >= n)) throw new RangeError("an index names a vertex the shape lacks");
    budget.bytes += vertices.byteLength + indices.byteLength;
    if (budget.bytes > LIMIT) throw new RangeError("scripts change at most 1 MiB of shapes a frame");
    return { format: f, stride, n, vertices, indices, visible: o.visible ?? true };
}

export function mesh(pool) {
    need("geometry");
    pool = String(pool);
    const def = pools[pool];
    if (!def) throw new TypeError(`${pool} is not a pool of the scene`);
    const shapes = (molds[pool] ??= new Map());
    const room = (skip, n) => {
        let total = n;
        for (const [id, s] of shapes) if (id !== skip) total += s.n;
        if (total > def.max_vertices) throw new RangeError(`${pool} holds at most ${def.max_vertices} vertices`);
    };
    const touch = () => [sent.add(pool), dirty.add("meshes")];
    const handle = (id) =>
        Object.freeze({
            id,
            get alive() {
                return shapes.has(id);
            },
            update(o) {
                const old = shapes.get(id);
                if (!old) throw new TypeError(`shape ${id} was destroyed`);
                const made = shape({ visible: old.visible, ...o }, old.format);
                room(id, made.n);
                shapes.set(id, made);
                touch();
            },
            destroy() {
                if (shapes.delete(id)) touch();
            },
        });
    return Object.freeze({
        create(o) {
            if (shapes.size >= def.max) throw new RangeError(`${pool} holds at most ${def.max} shapes`);
            const made = shape(o);
            room(null, made.n);
            const id = ++budget.count;
            shapes.set(id, made);
            touch();
            return handle(id);
        },
        get shapes() {
            return [...shapes.keys()].map(handle);
        },
        clear() {
            if (shapes.size) [shapes.clear(), touch()];
        },
    });
}

state.shapes = function takeShapes() {
    budget.bytes = 0;
    if (!sent.size) return null;
    const [out, room] = [[], { left: 3 << 20 }];
    for (const pool of [...sent]) {
        const shapes = [...molds[pool].values()].filter((s) => s.visible);
        room.left -= shapes.reduce((n, s) => n + s.vertices.byteLength + s.indices.byteLength, 0);
        if (room.left < 0 && out.length) break;
        out.push([pool, shapes.map((s) => [s.stride, s.vertices, s.indices])]);
        sent.delete(pool);
    }
    if (sent.size) dirty.add("meshes");
    return out;
};
