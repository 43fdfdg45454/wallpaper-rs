import { state, setup, channels } from "wprs:core";
import { color } from "wprs:values";

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
