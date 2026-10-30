const state = globalThis.__wprs;
delete globalThis.__wprs;
const setup = state.setup;
const [MAX_TIMERS, MAX_SPAWNED, MAX_WATCHED, MAX_LINES, MAX_LINE, MAX_SHARED, MAX_SIZE] = [64, 256, 64, 20, 512, 256, 65536];
const fields = ["position", "scale", "rotation", "opacity", "visible", "color", "text", "skew", "origin", "parallax", "size", "font", "align"];
const tunables = ["alpha", "size", "count", "speed", "life", "time_scale", "brightness", "color"];
const channels = ["left", "right", "average"];
const touches = ["click", "enter", "leave", "move", "down", "up"];
const lifecycle = ["init", "update", "destroy", "resize", "properties", "settings", "timelineEvent"];
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
};
Object.assign(state, { buffers: {}, once: false, fresh: false, loading: "" });

const records = new Map();
const [timers, handles, spawned, watched] = [new Map(), new Map(), new Map(), new Set()];
const [listeners, sites, pending, stopped, top] = [[], [], [], [], [...setup.top]];
const [handlers, touchers, overrides, controls, store, particleState] = [{}, {}, {}, {}, {}, {}];
let [current, out, timerId, spawnCount, ordered, started, lastTime, second, lines] = [null, null, 0, 0, false, false, 0, -1, 0];
const seen = { screen: "", settings: "", props: {} };

const record = (path) => {
    if (!records.has(path)) records.set(path, { path, failed: false, failures: 0, stopped: false, error: "" });
    return records.get(path);
};
const owner = () => current ?? record(state.loading);
const need = (api) => {
    const path = owner().path;
    if (setup.checked && !(setup.uses[path] ?? []).includes(api)) throw new TypeError(`${path} uses ${api} without declaring it in scene.json`);
};

function show(v) {
    if (typeof v === "string") return v;
    if (v instanceof Error) return `${v.name}: ${v.message}`;
    try {
        return JSON.stringify(v) ?? String(v);
    } catch {
        return String(v);
    }
}

function log(level, args) {
    const now = Math.floor(state.frame.wall);
    if (now !== second) [second, lines] = [now, 0];
    if (++lines > MAX_LINES) return;
    pending.push(`${level} ${owner().path}: ${args.map(show).join(" ")}`.slice(0, MAX_LINE));
}

function guard(mod, f, ...args) {
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

function stop(mod, why) {
    if (mod.stopped) return;
    mod.stopped = true;
    stopped.push(`${mod.path}: ${why}`.slice(0, MAX_LINE));
    for (const [id, t] of timers) if (t.owner === mod) timers.delete(id);
}

export const console = Object.freeze(Object.fromEntries(["log", "info", "warn", "error", "debug"].map((l) => [l, (...a) => log(l, a)])));

function schedule(fn, ms, repeat) {
    need("timers");
    if (typeof fn !== "function") throw new TypeError("a timer takes a function");
    if (timers.size >= MAX_TIMERS) throw new RangeError(`at most ${MAX_TIMERS} timers can wait at once`);
    const period = Math.max(0.016, (Number(ms) || 0) / 1000);
    const id = ++timerId;
    timers.set(id, { fn, due: state.frame.time + period, period: repeat ? period : 0, owner: owner() });
    return () => void timers.delete(id);
}

export const setTimeout = (fn, ms) => schedule(fn, ms, false);
export const setInterval = (fn, ms) => schedule(fn, ms, true);
export const clearTimeout = (cancel) => void (typeof cancel === "function" && cancel());
export const clearInterval = clearTimeout;
Object.assign(globalThis, { console, setTimeout, setInterval, clearTimeout, clearInterval });

export const shared = new Proxy(store, {
    set(target, key, value) {
        if (!(key in target) && Object.keys(target).length >= MAX_SHARED) throw new RangeError(`shared holds at most ${MAX_SHARED} values`);
        if ((typeof value === "string" || Array.isArray(value) || ArrayBuffer.isView(value)) && value.length > MAX_SIZE) {
            throw new RangeError(`a shared value holds at most ${MAX_SIZE} items`);
        }
        target[key] = value;
        return true;
    },
});

export const engine = Object.freeze({
    get time() {
        return state.frame.time;
    },
    get dt() {
        return state.frame.dt;
    },
    get locale() {
        return setup.locale;
    },
    get language() {
        return setup.locale;
    },
    get daytime() {
        return (((state.frame.wall % 86400) + 86400) % 86400) / 86400;
    },
    get fps() {
        return state.frame.fps;
    },
    get quality() {
        return state.frame.quality;
    },
    get canvas() {
        return [...setup.canvas];
    },
    get screen() {
        return [...state.frame.screen];
    },
    get device() {
        return setup.mobile ? "mobile" : "desktop";
    },
    get mode() {
        return state.frame.mode;
    },
    get orientation() {
        const [w, h] = state.frame.screen;
        return h > w ? "portrait" : "landscape";
    },
});

export const time = Object.freeze({
    format(pattern, date) {
        const local = date === undefined ? state.frame.wall : date.getTime() / 1000 - date.getTimezoneOffset() * 60;
        return state.format(String(pattern), Number(local), String(setup.locale));
    },
});

const channel = (hex, i) => (hex.length >= i + 2 ? parseInt(hex.slice(i, i + 2), 16) / 255 : 1);
const color = (text) => {
    const hex = String(text ?? "").replace("#", "");
    if (!/^[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(hex)) return undefined;
    return { r: channel(hex, 0), g: channel(hex, 2), b: channel(hex, 4), a: channel(hex, 6) };
};

export const props = Object.freeze({
    get: (id) => state.frame.props[id],
    all: () => ({ ...state.frame.props }),
    label: (id) => setup.labels[id]?.[String(state.frame.props[id])],
    pressed: (id) => state.frame.actions.includes(id),
    color: (id) => color(state.frame.props[id]),
});

const reduce = (all, count) => {
    const width = 64 / count;
    return Array.from({ length: count }, (_, i) => Math.max(...all.subarray(i * width, (i + 1) * width)));
};

export const audio = Object.freeze({
    bands(count = 64, which = "average") {
        if (![16, 32, 64].includes(count) || !channels.includes(which)) {
            throw new RangeError("bands takes 16, 32 or 64 and left, right or average");
        }
        return reduce(state.frame.audio[which], count);
    },
    buffers(count = 64) {
        if (![16, 32, 64].includes(count)) throw new RangeError("buffers takes 16, 32 or 64");
        if (!state.buffers[count]) {
            const made = Object.fromEntries(channels.map((c) => [c, Float32Array.from(reduce(state.frame.audio[c], count))]));
            state.buffers[count] = Object.freeze(made);
        }
        return state.buffers[count];
    },
    get bass() {
        return state.frame.audio.bass;
    },
    get mid() {
        return state.frame.audio.mid;
    },
    get treble() {
        return state.frame.audio.treble;
    },
    get level() {
        return state.frame.audio.level;
    },
    get beat() {
        return state.frame.audio.beat;
    },
});

export const input = Object.freeze({
    get cursor() {
        const { x, y, inside, buttons } = state.frame.cursor;
        const [[vx, vy, vw, vh], [sw, sh]] = [state.frame.view, state.frame.screen];
        return { x, y, inside, buttons, left: (buttons & 1) !== 0, scene: [vx + x * vw, vy + y * vh], screen: [x * sw, y * sh] };
    },
});

const numbers = (v, n, what) => {
    const list = typeof v === "string" && n === 4 ? Object.values(color(v) ?? {}) : v?.toArray ? v.toArray() : Array.isArray(v) || ArrayBuffer.isView(v) ? Array.from(v) : typeof v === "object" && v ? ("x" in v ? [v.x, v.y, v.z, v.w] : [v.r, v.g, v.b, v.a]) : [];
    const made = list.slice(0, n).filter((x) => x !== undefined).map(Number);
    if (n === 4 && made.length === 3) made.push(1);
    if (made.length !== n || !made.every(Number.isFinite)) throw new TypeError(`${what} takes ${n} numbers`);
    return made;
};
const number = (what) => (v) => {
    if (!Number.isFinite(Number(v))) throw new TypeError(`${what} takes a number`);
    return Number(v);
};
const pair = (what) => (v) => (typeof v === "number" ? [number(what)(v), v] : numbers(v, 2, what));
const convert = {
    position: (v) => numbers(v, 2, "position"),
    scale: pair("scale"),
    rotation: number("rotation"),
    opacity: number("opacity"),
    visible: Boolean,
    color: (v) => numbers(v, 4, "color"),
    text: String,
    skew: (v) => numbers(v, 2, "skew"),
    origin: (v) => numbers(v, 2, "origin"),
    parallax: pair("parallax"),
    size: (v) => (typeof v === "number" ? number("size")(v) : numbers(v, 2, "size")),
    font: String,
    align(v) {
        if (!["left", "center", "right"].includes(v)) throw new TypeError("align takes left, center or right");
        return v;
    },
};

const info = (id) => {
    if (setup.layers[id]) return setup.layers[id];
    const [head, ...rest] = id.split(".");
    return spawned.has(head) ? setup.templates[[spawned.get(head), ...rest].join(".")] : undefined;
};
const copy = (v) => (Array.isArray(v) ? [...v] : v);

function paramValue(def, value, id) {
    const clamp = (x) => Math.min(def.max ?? Infinity, Math.max(def.min ?? -Infinity, x));
    switch (def.kind) {
        case "float":
        case "enum":
            return clamp(number(id)(value));
        case "int":
            return clamp(Math.round(number(id)(value)));
        case "vec2":
        case "position":
            return (typeof value === "number" ? [value, value] : numbers(value, 2, id)).map(clamp);
        case "color":
            return numbers(value, 4, id);
        case "bool":
            return Boolean(value);
        case "blend":
            return String(value);
        default:
            throw new TypeError(`${id} cannot be set by a script`);
    }
}

function effect(id, which) {
    need("effects");
    const list = info(id)?.effects ?? [];
    const at = typeof which === "number" ? Math.trunc(which) : list.findIndex((e) => e.name === String(which));
    const e = list[at];
    if (!e) throw new RangeError(`${id} has no effect ${which}`);
    const control = ((controls[id] ??= {})[at] ??= { at, params: {}, reset: 0 });
    return Object.freeze({
        index: at,
        name: e.name ?? null,
        effect: e.effect,
        get enabled() {
            return control.enabled ?? true;
        },
        set enabled(on) {
            control.enabled = Boolean(on);
        },
        get: (param) => copy(control.params[param]),
        set(param, value) {
            const def = e.params[param];
            if (!def) throw new TypeError(`${e.effect} has no parameter ${param}`);
            control.params[param] = paramValue(def, value, param);
        },
        reset() {
            control.reset += 1;
        },
    });
}

function layer(id) {
    id = String(id);
    if (handles.has(id)) return handles.get(id);
    if (watched.size < MAX_WATCHED) watched.add(id);
    const h = {
        id,
        name: id,
        get type() {
            return info(id)?.type;
        },
        get initial() {
            return info(id)?.initial;
        },
        reset(key) {
            if (key === undefined) delete overrides[id];
            else delete overrides[id]?.[key];
        },
        on(event, fn) {
            if (!touches.includes(event) || typeof fn !== "function") {
                throw new TypeError(`on takes ${touches.join(", ")} and a function`);
            }
            ((touchers[id] ??= {})[event] ??= []).push([owner(), fn]);
        },
        effect: (which) => effect(id, which),
        destroy: () => scene.destroy(id),
    };
    for (const key of fields) {
        Object.defineProperty(h, key, {
            enumerable: true,
            get: () => copy(overrides[id]?.[key] ?? state.frame.reads[id]?.[key]),
            set(v) {
                (overrides[id] ??= {})[key] = convert[key](v);
            },
        });
    }
    handles.set(id, Object.freeze(h));
    return handles.get(id);
}

const spot = (index) => Math.max(0, Math.min(top.length, Math.trunc(Number(index) || 0)));

export const scene = Object.freeze({
    layer,
    layers: () => [...top],
    spawn(template, options = {}) {
        need("layers");
        template = String(template);
        if (!(template in setup.templates) || template.includes(".")) throw new TypeError(`${template} is not a template`);
        if (spawned.size >= MAX_SPAWNED) throw new RangeError(`at most ${MAX_SPAWNED} layers can be added`);
        let id = options.id === undefined ? "" : String(options.id);
        while (!id || (options.id === undefined && (spawned.has(id) || id in setup.layers))) id = `${template}_${++spawnCount}`;
        if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || spawned.has(id) || id in setup.layers || id in setup.templates) {
            throw new TypeError(`${id} is taken or is not a layer id`);
        }
        spawned.set(id, template);
        top.splice(options.at === undefined ? top.length : spot(options.at), 0, id);
        ordered ||= options.at !== undefined;
        const made = layer(id);
        for (const key of fields) if (options[key] !== undefined) made[key] = options[key];
        return made;
    },
    destroy(id) {
        need("layers");
        id = String(id);
        if (!spawned.has(id)) throw new TypeError(`${id} was not added by a script`);
        spawned.delete(id);
        top.splice(top.indexOf(id), 1);
        for (const map of [overrides, controls, touchers]) for (const k of Object.keys(map)) if (k === id || k.startsWith(`${id}.`)) delete map[k];
        for (const k of [...handles.keys()]) if (k === id || k.startsWith(`${id}.`)) [handles.delete(k), watched.delete(k)];
    },
    order(id, index) {
        need("layers");
        const from = top.indexOf(String(id));
        if (from < 0) throw new TypeError(`${id} is not a top-level layer`);
        top.splice(from, 1);
        top.splice(spot(index), 0, String(id));
        ordered = true;
    },
});

export function timeline(id) {
    id = String(id);
    const send = (command) => void out.timelines.push([id, command]);
    const now = () => state.frame.timelines?.[id] ?? {};
    return Object.freeze({
        play: () => send("play"),
        pause: () => send("pause"),
        reverse: () => send("reverse"),
        stop: () => send("stop"),
        seek: (t) => send({ seek: Number(t) }),
        on(name, handler) {
            if (typeof handler !== "function") throw new TypeError("on takes an event name and a function");
            ((handlers[id] ??= {})[String(name)] ??= []).push([owner(), handler]);
        },
        get time() {
            return now().time;
        },
        get duration() {
            return now().duration;
        },
        get playing() {
            return now().playing;
        },
        get speed() {
            return now().speed;
        },
        set speed(rate) {
            send({ speed: Number(rate) });
        },
    });
}

export function particles(id) {
    id = String(id);
    const own = (particleState[id] ??= { playing: true, instance: {}, points: {} });
    const send = (command) => void ((out.particles[id] ??= {}).commands ??= []).push(command);
    const whole = (n, lo, hi, what) => {
        n = Math.floor(Number(n));
        if (!(n >= lo && n <= hi)) throw new RangeError(`${what} takes ${lo} to ${hi}`);
        return n;
    };
    return Object.freeze({
        play() {
            own.playing = true;
            send("play");
        },
        pause() {
            own.playing = false;
            send("pause");
        },
        stop() {
            own.playing = false;
            send("stop");
        },
        get playing() {
            return own.playing;
        },
        emit: (n) => send({ emit: whole(n, 1, 1000, "emit") }),
        point(n, at) {
            own.points[whole(n, 0, 7, "point")] = numbers(at, 2, "point");
        },
        instance: new Proxy(
            {},
            {
                get: (_, key) => own.instance[key],
                set(_, key, value) {
                    if (!tunables.includes(key)) throw new TypeError(`${String(key)} is not an instance field`);
                    own.instance[key] = key === "color" ? numbers(value, 4, "color") : Number(value);
                    return true;
                },
            },
        ),
    });
}

const each = (name, ...args) => {
    for (const mod of records.values()) if (typeof mod.ns?.[name] === "function") guard(mod, mod.ns[name], ...args);
};

state.start = (roots) => {
    for (const [path, names, error] of roots) {
        const mod = record(path);
        mod.ns = names;
        mod.params = Object.freeze({ ...(setup.params[path] ?? {}) });
        if (!names) stop(mod, error ?? "it did not load");
        else if (typeof names.timelineEvent === "function") listeners.push([mod, names.timelineEvent]);
    }
    for (const site of setup.sites) {
        const mod = records.get(site.script);
        const target = mod?.ns?.[site.call];
        const fns = typeof target === "function" ? { update: target } : target && typeof target === "object" ? target : null;
        if (!mod || !fns || lifecycle.includes(site.call)) {
            pending.push(`error ${site.script}: ${site.path} names ${site.call}, which the script does not export`);
            continue;
        }
        const params = Object.freeze({ ...(setup.params[site.script] ?? {}), ...site.params });
        sites.push({ mod, fns, key: site.key, value: null, ctx: Object.freeze({ path: site.path, key: site.key, layer: site.layer, params }) });
    }
};

function events(f) {
    for (const e of f.events) {
        const event = Object.freeze({ timeline: e.timeline, name: e.name ?? "ended", t: e.t });
        if (e.name !== null && e.name !== undefined) listeners.forEach(([mod, fn]) => guard(mod, fn, event));
        (handlers[e.timeline]?.[event.name] ?? []).forEach(([mod, fn]) => guard(mod, fn, event));
    }
    for (const t of f.touches) {
        const type = touches[t.kind];
        const event = Object.freeze({ type, layer: layer(t.layer), local: t.local, scene: t.scene });
        (touchers[t.layer]?.[type] ?? []).forEach(([mod, fn]) => guard(mod, fn, event));
    }
}

function lifecycleEvents(f) {
    const screen = f.screen.join("x");
    const settings = { language: setup.locale, fps: f.fps, quality: f.quality };
    const props = f.props;
    if (!started) {
        started = true;
        for (const mod of records.values()) if (typeof mod.ns?.init === "function") guard(mod, mod.ns.init, Object.freeze({ params: mod.params, engine }));
        for (const s of sites) if (typeof s.fns.init === "function") s.value = guard(s.mod, s.fns.init, null, s.ctx) ?? null;
        each("properties", { ...props });
        each("settings", settings);
    } else {
        if (screen !== seen.screen) each("resize", { screen: [...f.screen], canvas: [...setup.canvas] });
        if (state.fresh) {
            const changed = Object.fromEntries(Object.entries(props).filter(([k, v]) => JSON.stringify(seen.props[k]) !== JSON.stringify(v)));
            if (Object.keys(changed).length) each("properties", changed);
        }
        if (JSON.stringify(settings) !== seen.settings) each("settings", settings);
    }
    [seen.screen, seen.settings, seen.props, state.fresh] = [screen, JSON.stringify(settings), props, false];
}

state.tick = () => {
    const f = state.frame;
    out = { values: {}, layers: overrides, timelines: [], particles: {}, effects: {}, spawned: [], watch: [], busy: false, console: [], stopped };
    if (f.time < lastTime) for (const t of timers.values()) t.due -= lastTime - f.time;
    lastTime = f.time;
    lifecycleEvents(f);
    for (const [id, t] of [...timers].filter(([, t]) => t.due <= f.time).sort((a, b) => a[1].due - b[1].due)) {
        if (t.period) t.due = f.time + t.period;
        else timers.delete(id);
        guard(t.owner, t.fn);
    }
    events(f);
    for (const mod of records.values()) {
        if (typeof mod.ns?.update !== "function" || mod.stopped) continue;
        out.busy = true;
        const value = guard(mod, mod.ns.update, f);
        if (value !== undefined) out.values[mod.path] = value;
    }
    for (const s of sites) {
        if (typeof s.fns.update === "function" && !s.mod.stopped) {
            out.busy = true;
            const value = guard(s.mod, s.fns.update, s.value, s.ctx);
            if (value !== undefined) s.value = value;
        }
        if (s.value !== null && s.value !== undefined) out.values[s.key] = s.value;
    }
    for (const [id, own] of Object.entries(particleState)) {
        Object.assign((out.particles[id] ??= {}), { instance: own.instance, points: own.points });
    }
    out.effects = Object.fromEntries(Object.entries(controls).map(([id, list]) => [id, Object.values(list)]));
    out.spawned = [...spawned];
    if (ordered) out.order = [...top];
    out.watch = [...watched];
    let soon = Infinity;
    for (const t of timers.values()) soon = Math.min(soon, t.due - f.time);
    if (soon !== Infinity) out.wake = Math.max(0, soon);
    for (const mod of records.values()) {
        mod.failures = mod.failed ? mod.failures + 1 : 0;
        mod.failed = false;
        if (mod.failures >= 3) stop(mod, mod.error);
    }
    out.console = pending.splice(0);
    state.once = out.timelines.length > 0 || out.console.length > 0 || Object.values(out.particles).some((p) => p.commands?.length);
    return out;
};

state.destroy = () => {
    each("destroy");
    timers.clear();
};
