import { engine as e, props as p, audio as a, input as i } from "wprs:base";
import { own } from "wprs:layers";
import { copy, convert } from "wprs:values";
export { time } from "wprs:base";
export { timeline, particles } from "wprs:media";

const fields = ["position", "scale", "rotation", "opacity", "visible", "color", "text"];
const only = (from, keys) => Object.freeze(Object.defineProperties({}, Object.fromEntries(keys.map((k) => [k, { enumerable: true, get: () => from[k] }]))));
const views = new Map();

export const engine = only(e, ["time", "dt", "locale", "daytime"]);
export const props = only(p, ["get", "label", "pressed", "color"]);
export const audio = only(a, ["bands", "bass", "mid", "treble", "level", "beat"]);
export const input = Object.freeze({
    get cursor() {
        const { x, y, inside, buttons } = i.cursor;
        return { x, y, inside, buttons };
    },
});
export const scene = Object.freeze({
    layer(id) {
        id = String(id);
        if (!views.has(id)) {
            const field = (k) => [k, { enumerable: true, get: () => copy(own.once[id]?.[k]), set: (v) => void ((own.once[id] ??= {})[k] = convert[k](v)) }];
            views.set(id, Object.freeze(Object.defineProperties({}, Object.fromEntries(fields.map(field)))));
        }
        return views.get(id);
    },
});
