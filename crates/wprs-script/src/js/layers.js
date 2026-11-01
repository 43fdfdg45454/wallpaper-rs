import { state, setup, MAX, owner, need, dirty, touched, put } from "wprs:core";
import { fields, convert, copy, paramValue } from "wprs:values";
import { anim, video } from "wprs:media";
import { family } from "wprs:kin";

export const touches = ["click", "enter", "leave", "move", "down", "up"];
export const own = { handles: new Map(), spawned: new Map(), watched: new Set(), touchers: {}, overrides: {}, once: {}, controls: {}, top: [...setup.top], ordered: false, count: 0, size: 0 };
const sizes = new Map();
const drawn = Object.keys(setup.layers).length;

export const info = (id) => {
    if (setup.layers[id]) return setup.layers[id];
    const [head, ...rest] = id.split(".");
    return own.spawned.has(head) ? setup.templates[[own.spawned.get(head), ...rest].join(".")] : undefined;
};
export const watch = (id) => void (own.watched.size < MAX.watched && !own.watched.has(id) && own.watched.add(id) && dirty.add("watch"));
const sized = (template) => {
    if (!sizes.has(template)) sizes.set(template, Object.keys(setup.templates).filter((k) => k === template || k.startsWith(`${template}.`)).length);
    return sizes.get(template);
};

function effect(id, which) {
    need("effects");
    const list = info(id)?.effects ?? [];
    const at = typeof which === "number" ? Math.trunc(which) : list.findIndex((e) => e.name === String(which));
    const e = list[at];
    if (!e) throw new RangeError(`${id} has no effect ${which}`);
    if (!own.controls[id]?.[at]) dirty.add("effects");
    const control = ((own.controls[id] ??= {})[at] ??= { at, params: {}, reset: 0 });
    return Object.freeze({
        index: at,
        name: e.name ?? null,
        effect: e.effect,
        get enabled() {
            return control.enabled ?? true;
        },
        set enabled(on) {
            if (put(control, "enabled", Boolean(on))) dirty.add("effects");
        },
        get: (param) => copy(control.params[param]),
        set(param, value) {
            const def = e.params[param];
            if (!def) throw new TypeError(`${e.effect} has no parameter ${param}`);
            if (put(control.params, param, paramValue(def, value, param))) dirty.add("effects");
        },
        reset() {
            control.reset += 1;
            dirty.add("effects");
        },
    });
}

export function layer(id, watching = true) {
    id = String(id);
    if (watching && !state.started) watch(id);
    if (own.handles.has(id)) return own.handles.get(id);
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
            if (own.overrides[id] && (key === undefined || key in own.overrides[id])) touched.add(id);
            if (key === undefined) delete own.overrides[id];
            else delete own.overrides[id]?.[key];
        },
        on(event, fn) {
            if (!touches.includes(event) || typeof fn !== "function") throw new TypeError(`on takes ${touches.join(", ")} and a function`);
            ((own.touchers[id] ??= {})[event] ??= []).push([owner(), fn]);
        },
        effect: (which) => effect(id, which),
        get video() {
            return video(id);
        },
        anim: (clip) => anim(id, clip),
        destroy: () => scene.destroy(id),
    };
    family(h, id);
    for (const key of fields) {
        Object.defineProperty(h, key, {
            enumerable: true,
            get() {
                watch(id);
                return copy(own.overrides[id]?.[key] ?? state.frame.reads[id]?.[key]);
            },
            set(v) {
                if (put((own.overrides[id] ??= {}), key, convert[key](v))) touched.add(id);
            },
        });
    }
    own.handles.set(id, Object.freeze(h));
    return own.handles.get(id);
}

const spot = (index) => Math.max(0, Math.min(own.top.length, Math.trunc(Number(index) || 0)));

export const scene = {
    layer: (id) => layer(id),
    layers: () => [...own.top],
    spawn(template, options = {}) {
        need("layers");
        template = String(template);
        if (!(template in setup.templates) || template.includes(".")) throw new TypeError(`${template} is not a template`);
        if (own.spawned.size >= MAX.spawned) throw new RangeError(`at most ${MAX.spawned} layers can be added`);
        if (drawn + own.size + sized(template) > MAX.layers) throw new RangeError(`the scene and the layers added to it hold at most ${MAX.layers} layers`);
        let id = options.id === undefined ? "" : String(options.id);
        while (!id || (options.id === undefined && (own.spawned.has(id) || id in setup.layers))) id = `${template}_${++own.count}`;
        if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || own.spawned.has(id) || id in setup.layers || id in setup.templates) {
            throw new TypeError(`${id} is taken or is not a layer id`);
        }
        own.spawned.set(id, template);
        own.size += sized(template);
        own.top.splice(options.at === undefined ? own.top.length : spot(options.at), 0, id);
        own.ordered ||= options.at !== undefined;
        ["spawned", "order"].forEach((k) => dirty.add(k));
        const made = layer(id);
        for (const key of fields) if (options[key] !== undefined) made[key] = options[key];
        return made;
    },
    destroy(id) {
        need("layers");
        id = String(id);
        if (!own.spawned.has(id)) throw new TypeError(`${id} was not added by a script`);
        own.size -= sized(own.spawned.get(id));
        own.spawned.delete(id);
        own.top.splice(own.top.indexOf(id), 1);
        const mine = (k) => k === id || k.startsWith(`${id}.`);
        for (const k of Object.keys(own.overrides)) if (mine(k)) touched.add(k);
        for (const map of [own.overrides, own.controls, own.touchers]) for (const k of Object.keys(map)) if (mine(k)) delete map[k];
        for (const k of [...own.handles.keys()]) if (mine(k)) [own.handles.delete(k), own.watched.delete(k)];
        ["effects", "spawned", "order", "watch"].forEach((k) => dirty.add(k));
    },
    order(id, index) {
        need("layers");
        const from = own.top.indexOf(String(id));
        if (from < 0) throw new TypeError(`${id} is not a top-level layer`);
        own.top.splice(from, 1);
        own.top.splice(spot(index), 0, String(id));
        own.ordered = true;
        dirty.add("order");
    },
};
