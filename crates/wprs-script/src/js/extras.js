import { state, MAX, timers, owner, newer, need, log } from "wprs:core";

const say = (level) => (...args) => {
    newer("console");
    log(level, args);
};
export const console = Object.freeze(Object.fromEntries(["log", "info", "warn", "error", "debug"].map((l) => [l, say(l)])));

let timerId = 0;
function schedule(fn, ms, repeat) {
    need("timers");
    if (typeof fn !== "function") throw new TypeError("a timer takes a function");
    if (timers.size >= MAX.timers) throw new RangeError(`at most ${MAX.timers} timers can wait at once`);
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

export function openShortcut(id) {
    need("shortcuts");
    const prop = String(id);
    if (!state.clicking || state.out.open.length >= 4) return false;
    if (!state.out.open.includes(prop)) state.out.open.push(prop);
    return true;
}

const refused = [Map, Set, WeakMap, WeakSet, Promise, Date, RegExp, DataView, ArrayBuffer];
function frozen(value) {
    let items = 0;
    const take = (n) => {
        items += n;
        if (items > MAX.size) throw new RangeError(`a shared value holds at most ${MAX.size} items`);
    };
    const walk = (v, depth) => {
        if (typeof v === "function" || typeof v === "symbol" || refused.some((k) => v instanceof k)) {
            throw new TypeError("shared holds numbers, strings, booleans, arrays, typed arrays and objects of them");
        }
        if (v === null || typeof v !== "object") return take(typeof v === "string" ? Math.max(1, v.length) : 1), v;
        if (depth > 32) throw new RangeError("a shared value nests at most 32 levels");
        if (ArrayBuffer.isView(v)) return take(Math.max(1, v.length)), v.slice();
        if (Array.isArray(v)) return take(Math.max(1, v.length)), Object.freeze(Array.from(v, (x) => walk(x, depth + 1)));
        const made = Object.create(Object.getPrototypeOf(v));
        const keys = Object.keys(v);
        take(Math.max(1, keys.length));
        for (const k of keys) made[k] = walk(v[k], depth + 1);
        return Object.freeze(made);
    };
    return walk(value, 0);
}

const room = (target, key) => {
    if (!(key in target) && Object.keys(target).length >= MAX.shared) throw new RangeError(`shared holds at most ${MAX.shared} values`);
};
export const shared = new Proxy(
    {},
    {
        set(target, key, value) {
            room(target, key);
            target[key] = frozen(value);
            return true;
        },
        defineProperty(target, key, desc) {
            if ("get" in desc || "set" in desc) throw new TypeError("shared holds values, not getters or setters");
            room(target, key);
            return Reflect.defineProperty(target, key, { configurable: true, enumerable: true, writable: true, value: frozen(desc.value) });
        },
        setPrototypeOf: () => false,
        preventExtensions: () => false,
    },
);
