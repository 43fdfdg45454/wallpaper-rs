import { state, setup, channels, records, timers, pending, stopped, record, guard, stop, dirty, touched, SECTIONS } from "wprs:core";
import { engine } from "wprs:base";
import { own, layer, touches } from "wprs:layers";
import { handlers, particleState, soundState, videoHandlers } from "wprs:media";

const lifecycle = ["init", "update", "destroy", "resize", "properties", "settings", "timelineEvent"];
const [listeners, sites] = [[], []];
const seen = { screen: "", fps: -1, quality: -1, props: {} };
let [lastTime, legacy] = [0, null];
const last = { values: undefined, busy: undefined, wake: undefined, loud: {}, once: [] };
const plain = (v) => (v !== null && typeof v === "object" && typeof v.toJSON === "function" ? v.toJSON() : v);
const kept = (v) => ((v = plain(v)) === null || typeof v !== "object" ? v : Array.isArray(v) ? v.map(kept) : Object.fromEntries(Object.entries(v).map(([k, x]) => [k, kept(x)])));
const same = (a, b) => {
    a = plain(a);
    if (Object.is(a, b)) return true;
    if (a === null || b === null || typeof a !== "object" || typeof b !== "object" || Array.isArray(a) !== Array.isArray(b)) return false;
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every((k) => k in b && same(a[k], b[k]));
};

const each = (name, ...args) => {
    for (const mod of records.values()) if (!mod.first && typeof mod.ns?.[name] === "function") guard(mod, mod.ns[name], ...args);
};

const old = (f) =>
    (legacy ??= {
        time: f.time,
        dt: f.dt,
        wall: f.wall,
        zone: f.zone,
        props: { ...f.props },
        actions: [...f.actions],
        cursor: { ...f.cursor },
        audio: { ...f.audio, ...Object.fromEntries(channels.map((c) => [c, Array.from(f.audio[c])])) },
        timelines: Object.fromEntries(Object.entries(f.timelines).map(([k, v]) => [k, { ...v }])),
        events: f.events.map((e) => ({ ...e })),
    });

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
    for (const id of f.ended) {
        const event = Object.freeze({ layer: id, name: "ended" });
        (videoHandlers[id]?.ended ?? []).forEach(([mod, fn]) => guard(mod, fn, event));
    }
    for (const t of f.touches) {
        const type = touches[t.kind];
        const event = Object.freeze({ type, layer: layer(t.layer), local: t.local, scene: t.scene });
        state.clicking = type === "click";
        (own.touchers[t.layer]?.[type] ?? []).forEach(([mod, fn]) => guard(mod, fn, event));
        state.clicking = false;
    }
}

function lifecycleEvents(f) {
    const screen = `${f.screen[0]}x${f.screen[1]}`;
    const settings = f.fps !== seen.fps || f.quality !== seen.quality;
    if (!state.started) {
        for (const mod of records.values()) if (!mod.first && typeof mod.ns?.init === "function") guard(mod, mod.ns.init, Object.freeze({ params: mod.params, engine }));
        for (const s of sites) if (typeof s.fns.init === "function") s.value = guard(s.mod, s.fns.init, null, s.ctx) ?? null;
        state.started = true;
        each("properties", { ...f.props });
    } else {
        if (screen !== seen.screen) each("resize", { screen: [...f.screen], canvas: [...setup.canvas] });
        if (state.fresh) {
            const changed = Object.fromEntries(Object.entries(f.props).filter(([k, v]) => !same(v, seen.props[k])));
            if (Object.keys(changed).length) each("properties", changed);
        }
    }
    if (settings) each("settings", { language: setup.locale, fps: f.fps, quality: f.quality });
    [seen.screen, seen.fps, seen.quality, seen.props, state.fresh] = [screen, f.fps, f.quality, f.props, false];
}

state.tick = () => {
    const f = state.frame;
    const out = (state.out = { timelines: [], particles: {}, sounds: {}, videos: {}, open: [] });
    const [values, run] = [{}, { busy: false }];
    [legacy, own.once] = [null, {}];
    for (const s of Object.values(soundState)) s.asked = null;
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
        run.busy = true;
        const value = guard(mod, mod.ns.update, mod.first ? old(f) : f);
        if (value !== undefined) values[mod.path] = value;
    }
    for (const s of sites) {
        if (typeof s.fns.update === "function" && !s.mod.stopped) {
            run.busy = true;
            const value = guard(s.mod, s.fns.update, s.value, s.ctx);
            if (value !== undefined) s.value = value;
        }
        if (s.value !== null && s.value !== undefined) values[s.key] = s.value;
    }
    let soon = Infinity;
    for (const t of timers.values()) soon = Math.min(soon, t.due - f.time);
    run.wake = soon === Infinity ? null : Math.max(0, soon);
    for (const mod of records.values()) {
        mod.failures = mod.failed ? mod.failures + 1 : 0;
        mod.failed = false;
        if (mod.failures >= 3) stop(mod, mod.error);
    }
    const lines = pending.splice(0);
    if (!same(values, last.values)) [last.values] = [kept(values), dirty.add("values")];
    for (const k of ["busy", "wake"]) if (!Object.is(run[k], last[k])) [last[k]] = [run[k], dirty.add(k)];
    const commands = (list) => Object.values(list).some((p) => p.commands?.length);
    const loud = { timelines: out.timelines.length > 0, open: out.open.length > 0, console: lines.length > 0, particles: commands(out.particles), sounds: commands(out.sounds), videos: commands(out.videos) };
    for (const id of [...last.once, ...Object.keys(own.once)]) touched.add(id);
    last.once = Object.keys(own.once);
    for (const k of Object.keys(loud)) if (loud[k] || last.loud[k]) dirty.add(k);
    last.loud = loud;
    if (!dirty.size && !touched.size) return null;
    const layer = (id) => (own.once[id] ? { ...own.overrides[id], ...own.once[id] } : own.overrides[id]) ?? null;
    const made = {
        values: () => values,
        layers: () => Object.fromEntries([...new Set([...Object.keys(own.overrides), ...last.once])].map((id) => [id, layer(id)]).filter(([, v]) => v)),
        timelines: () => out.timelines,
        open: () => out.open,
        particles: () => Object.assign(out.particles, Object.fromEntries(Object.entries(particleState).map(([id, p]) => [id, { ...out.particles[id], instance: p.instance, points: p.points }]))),
        sounds: () => Object.assign(out.sounds, Object.fromEntries(Object.entries(soundState).map(([id, s]) => [id, { ...out.sounds[id], volume: s.volume }]))),
        videos: () => out.videos,
        effects: () => Object.fromEntries(Object.entries(own.controls).map(([id, list]) => [id, Object.values(list)])),
        spawned: () => [...own.spawned],
        order: () => (own.ordered ? [...own.top] : null),
        watch: () => [...own.watched],
        wake: () => run.wake,
        busy: () => run.busy,
        console: () => lines,
        stopped: () => stopped,
    };
    const changed = Object.fromEntries([...dirty].map((k) => [k, made[k]()]));
    if (touched.size && !dirty.has("layers")) changed.layer_edits = Object.fromEntries([...touched].map((id) => [id, layer(id)]));
    [dirty, touched].forEach((set) => set.clear());
    return changed;
};

state.forget = () => {
    for (const k of SECTIONS) dirty.add(k);
};

state.destroy = () => {
    each("destroy");
    timers.clear();
};
