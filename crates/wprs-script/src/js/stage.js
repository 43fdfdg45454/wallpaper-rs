import { setup, need, dirty } from "wprs:core";
import { scene } from "wprs:layers";
import { numbers, number } from "wprs:values";
import { mesh } from "wprs:molds";

export const stage = {};
const initial = setup.stage ?? {};
const vec = (what) => (v) => numbers(v, 2, what);
const fields = (shape) => (v) => {
    if (v === null || typeof v !== "object" || Array.isArray(v)) throw new TypeError(`takes an object with ${Object.keys(shape).join(", ")}`);
    const unknown = Object.keys(v).find((k) => !(k in shape));
    if (unknown) throw new TypeError(`${unknown} is not one of ${Object.keys(shape).join(", ")}`);
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, shape[k](x)]));
};
const shake = fields({ enabled: Boolean, amplitude: number("amplitude"), roughness: number("roughness"), speed: number("speed") });
const shapes = {
    camera: fields({ center: vec("center"), zoom: number("zoom"), rotation: number("rotation"), shake }),
    parallax: fields({ amount: number("amount"), delay: number("delay") }),
    bloom: fields({ threshold: number("threshold"), strength: number("strength"), radius: number("radius"), tint: (v) => numbers(v, 4, "tint") }),
};
const colors = ["background", "ambient", "sky"];
const plain = (v) => (v !== null && typeof v === "object" && !Array.isArray(v) ? Object.fromEntries(Object.entries(v).filter(([, x]) => typeof x !== "object" || Array.isArray(x))) : {});
const read = (key) => {
    const set = stage[key] ?? {};
    const shaken = key === "camera" && (set.shake || initial.camera?.shake) ? { shake: { ...plain(initial.camera?.shake), ...set.shake } } : {};
    return Object.freeze({ ...plain(initial[key]), ...set, ...shaken });
};
const write = (key, value) => {
    need("camera");
    if (value === null || value === undefined) delete stage[key];
    else if (colors.includes(key)) stage[key] = numbers(value, 4, key);
    else {
        const made = shapes[key](value);
        if (made.shake) made.shake = { ...stage[key]?.shake, ...made.shake };
        stage[key] = { ...stage[key], ...made };
    }
    dirty.add("stage");
};

Object.defineProperties(scene, {
    ...Object.fromEntries(colors.map((key) => [key, { enumerable: true, get: () => (need("camera"), stage[key] ?? initial[key]), set: (v) => write(key, v) }])),
    ...Object.fromEntries(["camera", "parallax", "bloom"].map((key) => [key, { enumerable: true, get: () => (need("camera"), read(key)), set: (v) => write(key, v) }])),
    mesh: { value: mesh },
});
Object.freeze(scene);
export { scene };
