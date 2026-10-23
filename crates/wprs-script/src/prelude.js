const state = globalThis.__wprs;
delete globalThis.__wprs;

const fields = ["position", "scale", "rotation", "opacity", "visible", "color", "text"];
const channels = ["left", "right", "average"];

export const engine = Object.freeze({
    get time() {
        return state.frame.time;
    },
    get dt() {
        return state.frame.dt;
    },
    get locale() {
        return state.locale;
    },
});

const channel = (hex, i) => (hex.length >= i + 2 ? parseInt(hex.slice(i, i + 2), 16) / 255 : 1);

export const props = Object.freeze({
    get: (id) => state.frame.props[id],
    label: (id) => state.labels[id]?.[String(state.frame.props[id])],
    pressed: (id) => state.frame.actions.includes(id),
    color(id) {
        const hex = String(state.frame.props[id] ?? "").replace("#", "");
        if (!/^[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(hex)) return undefined;
        return { r: channel(hex, 0), g: channel(hex, 2), b: channel(hex, 4), a: channel(hex, 6) };
    },
});

export const audio = Object.freeze({
    bands(count = 64, channel = "average") {
        if (![16, 32, 64].includes(count) || !channels.includes(channel)) {
            throw new RangeError("bands takes 16, 32 or 64 and left, right or average");
        }
        const all = state.frame.audio[channel];
        const width = 64 / count;
        return Array.from({ length: count }, (_, i) => Math.max(...all.slice(i * width, (i + 1) * width)));
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
        return { ...state.frame.cursor };
    },
});

export const scene = Object.freeze({
    layer(id) {
        return new Proxy(
            {},
            {
                get: (_, key) => state.out.layers[id]?.[key],
                set(_, key, value) {
                    if (!fields.includes(key)) throw new TypeError(`${String(key)} is not a layer field`);
                    (state.out.layers[id] ??= {})[key] = value;
                    return true;
                },
            },
        );
    },
});

export function timeline(id) {
    const send = (command) => void state.out.timelines.push([String(id), command]);
    return Object.freeze({
        play: () => send("play"),
        pause: () => send("pause"),
        reverse: () => send("reverse"),
        seek: (t) => send({ seek: Number(t) }),
    });
}
