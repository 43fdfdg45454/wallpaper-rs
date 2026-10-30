import { state, owner, need, dirty, put } from "wprs:core";
import { numbers } from "wprs:values";

const tunables = ["alpha", "size", "count", "speed", "life", "time_scale", "brightness", "color"];
export const [handlers, particleState, soundState] = [{}, {}, {}];

export function timeline(id) {
    id = String(id);
    const send = (command) => void state.out.timelines.push([id, command]);
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
    if (!particleState[id]) dirty.add("particles");
    const own = (particleState[id] ??= { playing: true, instance: {}, points: {} });
    const send = (command) => void ((state.out.particles[id] ??= {}).commands ??= []).push(command);
    const whole = (n, lo, hi, what) => {
        n = Math.floor(Number(n));
        if (!(n >= lo && n <= hi)) throw new RangeError(`${what} takes ${lo} to ${hi}`);
        return n;
    };
    const playing = (on, command) => {
        own.playing = on;
        send(command);
    };
    return Object.freeze({
        play: () => playing(true, "play"),
        pause: () => playing(false, "pause"),
        stop: () => playing(false, "stop"),
        get playing() {
            return own.playing;
        },
        emit: (n) => send({ emit: whole(n, 1, 1000, "emit") }),
        point(n, at) {
            if (put(own.points, whole(n, 0, 7, "point"), numbers(at, 2, "point"))) dirty.add("particles");
        },
        instance: new Proxy(
            {},
            {
                get: (_, key) => own.instance[key],
                set(_, key, value) {
                    if (!tunables.includes(key)) throw new TypeError(`${String(key)} is not an instance field`);
                    if (put(own.instance, key, key === "color" ? numbers(value, 4, "color") : Number(value))) dirty.add("particles");
                    return true;
                },
            },
        ),
    });
}

export function sound(id) {
    need("sounds");
    id = String(id);
    if (!soundState[id]) dirty.add("sounds");
    const own = (soundState[id] ??= { volume: 1, asked: null });
    const send = (command) => {
        ((state.out.sounds[id] ??= {}).commands ??= []).push(command);
        own.asked = command === "play";
    };
    return Object.freeze({
        play: () => send("play"),
        pause: () => send("pause"),
        stop: () => send("stop"),
        get playing() {
            return own.asked ?? state.frame.sounds.includes(id);
        },
        get volume() {
            return own.volume;
        },
        set volume(v) {
            if (!Number.isFinite(Number(v))) throw new TypeError("volume takes a number from 0 to 1");
            if (put(own, "volume", Math.min(1, Math.max(0, Number(v))))) dirty.add("sounds");
        },
    });
}
