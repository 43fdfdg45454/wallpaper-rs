import { state, setup, channels, records, timers, pending, stopped, record, guard, stop } from "wprs:core";
import { engine } from "wprs:base";
import { own, layer, touches } from "wprs:layers";
import { handlers, particleState, soundState } from "wprs:media";

const lifecycle = ["init", "update", "destroy", "resize", "properties", "settings", "timelineEvent"];
const [listeners, sites] = [[], []];
const seen = { screen: "", fps: -1, quality: -1, props: {} };
let [lastTime, legacy] = [0, null];

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
    for (const t of f.touches) {
        const type = touches[t.kind];
        const event = Object.freeze({ type, layer: layer(t.layer), local: t.local, scene: t.scene });
        (own.touchers[t.layer]?.[type] ?? []).forEach(([mod, fn]) => guard(mod, fn, event));
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
            const changed = Object.fromEntries(Object.entries(f.props).filter(([k, v]) => JSON.stringify(seen.props[k]) !== JSON.stringify(v)));
            if (Object.keys(changed).length) each("properties", changed);
        }
    }
    if (settings) each("settings", { language: setup.locale, fps: f.fps, quality: f.quality });
    [seen.screen, seen.fps, seen.quality, seen.props, state.fresh] = [screen, f.fps, f.quality, f.props, false];
}

state.tick = () => {
    const f = state.frame;
    const out = (state.out = { values: {}, layers: own.overrides, timelines: [], particles: {}, sounds: {}, effects: {}, spawned: [], watch: [], busy: false, console: [], stopped });
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
        out.busy = true;
        const value = guard(mod, mod.ns.update, mod.first ? old(f) : f);
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
    for (const [id, p] of Object.entries(particleState)) Object.assign((out.particles[id] ??= {}), { instance: p.instance, points: p.points });
    for (const [id, s] of Object.entries(soundState)) (out.sounds[id] ??= {}).volume = s.volume;
    const once = Object.entries(own.once);
    if (once.length) out.layers = { ...own.overrides, ...Object.fromEntries(once.map(([id, o]) => [id, { ...own.overrides[id], ...o }])) };
    out.effects = Object.fromEntries(Object.entries(own.controls).map(([id, list]) => [id, Object.values(list)]));
    out.spawned = [...own.spawned];
    if (own.ordered) out.order = [...own.top];
    out.watch = [...own.watched];
    let soon = Infinity;
    for (const t of timers.values()) soon = Math.min(soon, t.due - f.time);
    if (soon !== Infinity) out.wake = Math.max(0, soon);
    for (const mod of records.values()) {
        mod.failures = mod.failed ? mod.failures + 1 : 0;
        mod.failed = false;
        if (mod.failures >= 3) stop(mod, mod.error);
    }
    out.console = pending.splice(0);
    state.once = out.timelines.length > 0 || out.console.length > 0 || Object.values(out.particles).some((p) => p.commands?.length) || Object.values(out.sounds).some((p) => p.commands?.length);
    return out;
};

state.destroy = () => {
    each("destroy");
    timers.clear();
};
