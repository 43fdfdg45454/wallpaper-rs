export const state = globalThis.__wprs;
delete globalThis.__wprs;
export const setup = state.setup;
export const MAX = { timers: 64, spawned: 256, watched: 64, lines: 20, line: 512, shared: 256, size: 65536, layers: 512 };
export const channels = ["left", "right", "average"];
const bands = () => Object.fromEntries(channels.map((c) => [c, new Float32Array(64)]));

state.frame = {
    time: 0,
    dt: 0,
    wall: 0,
    zone: "",
    props: {},
    actions: [],
    screen: [...setup.canvas],
    view: [0, 0, ...setup.canvas],
    fps: 60,
    quality: 3,
    mode: "wallpaper",
    cursor: { x: 0.5, y: 0.5, inside: false, buttons: 0 },
    audio: { ...bands(), bass: 0, mid: 0, treble: 0, level: 0, beat: false },
    timelines: {},
    events: [],
    reads: {},
    touches: [],
    sounds: [],
    videos: {},
    ended: [],
};
Object.assign(state, { buffers: {}, fresh: false, loading: "", started: false, out: null });

export const records = new Map();
export const timers = new Map();
export const [pending, stopped] = [[], []];
export const SECTIONS = ["values", "layers", "timelines", "particles", "sounds", "videos", "effects", "spawned", "order", "watch", "wake", "busy", "console", "stopped", "open", "stage", "storage", "meshes", "rigs"];
export const [dirty, touched] = [new Set(SECTIONS), new Set()];
const equal = (a, b) => Object.is(a, b) || (Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((x, i) => Object.is(x, b[i])));
export const put = (target, key, value) => !equal(target[key], value) && ((target[key] = value), true);
const first = new Set(setup.first ?? []);
let [current, second, lines] = [null, -1, 0];

export const record = (path) => {
    if (!records.has(path)) records.set(path, { path, first: first.has(path), failed: false, failures: 0, stopped: false, error: "" });
    return records.get(path);
};
export const owner = () => current ?? record(state.loading);
export const newer = (what) => {
    const mod = owner();
    if (mod.first) throw new TypeError(`${mod.path} uses ${what}, which a module has to declare in scene.json`);
};
export const need = (api) => {
    newer(api);
    const path = owner().path;
    if (setup.checked && !(setup.uses[path] ?? []).includes(api)) throw new TypeError(`${path} uses ${api} without declaring it in scene.json`);
};

const plain = (text) => text.replace(/[\u0000-\u001f\u007f-\u009f]/g, " ").slice(0, MAX.line);

export function show(v) {
    if (typeof v === "string") return v;
    if (v instanceof Error) return `${v.name}: ${v.message}`;
    try {
        return JSON.stringify(v) ?? String(v);
    } catch {
        return String(v);
    }
}

export function log(level, args) {
    const now = Math.floor(state.frame.wall);
    if (now !== second) [second, lines] = [now, 0];
    if (++lines > MAX.lines) return;
    pending.push(plain(`${level} ${owner().path}: ${args.map(show).join(" ")}`));
}

export function guard(mod, f, ...args) {
    if (mod.stopped) return undefined;
    const outer = current;
    current = mod;
    try {
        return f(...args);
    } catch (e) {
        mod.failed = true;
        mod.error = show(e);
        log("error", [e]);
        return undefined;
    } finally {
        current = outer;
    }
}

export function stop(mod, why) {
    if (mod.stopped) return;
    mod.stopped = true;
    stopped.push(plain(`${mod.path}: ${why}`));
    dirty.add("stopped");
    for (const [id, t] of timers) if (t.owner === mod) timers.delete(id);
}
