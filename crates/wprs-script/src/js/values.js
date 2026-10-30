export const fields = ["position", "scale", "rotation", "opacity", "visible", "color", "text", "skew", "origin", "parallax", "size", "font", "align"];

const channel = (hex, i) => (hex.length >= i + 2 ? parseInt(hex.slice(i, i + 2), 16) / 255 : 1);
export const color = (text) => {
    const hex = String(text ?? "").replace("#", "");
    if (!/^[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(hex)) return undefined;
    return { r: channel(hex, 0), g: channel(hex, 2), b: channel(hex, 4), a: channel(hex, 6) };
};

export const numbers = (v, n, what) => {
    const list = typeof v === "string" && n === 4 ? Object.values(color(v) ?? {}) : v?.toArray ? v.toArray() : Array.isArray(v) || ArrayBuffer.isView(v) ? Array.from(v) : typeof v === "object" && v ? ("x" in v ? [v.x, v.y, v.z, v.w] : [v.r, v.g, v.b, v.a]) : [];
    const made = list.slice(0, n).filter((x) => x !== undefined).map(Number);
    if (n === 4 && made.length === 3) made.push(1);
    if (made.length !== n || !made.every(Number.isFinite)) throw new TypeError(`${what} takes ${n} numbers`);
    return made;
};
export const number = (what) => (v) => {
    if (!Number.isFinite(Number(v))) throw new TypeError(`${what} takes a number`);
    return Number(v);
};
const pair = (what) => (v) => (typeof v === "number" ? [number(what)(v), v] : numbers(v, 2, what));
export const convert = {
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
export const copy = (v) => (Array.isArray(v) ? [...v] : v);

export function paramValue(def, value, id) {
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
