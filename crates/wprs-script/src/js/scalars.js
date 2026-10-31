export const KEYS = ["x", "y", "z", "w"];
export const COLOR = ["r", "g", "b", "a"];

export const list = (v) => {
    if (v?.toArray) return v.toArray();
    if (Array.isArray(v) || ArrayBuffer.isView(v)) return Array.from(v, Number);
    if (v && typeof v === "object") return (("x" in v) ? KEYS : COLOR).map((k) => v[k]).filter((c) => c !== undefined).map(Number);
    return null;
};

export const degrees = (r) => (r * 180) / Math.PI;
export const radians = (d) => (d * Math.PI) / 180;
export const clamp = (x, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, x));
export const step = (edge, x) => (x < edge ? 0 : 1);
export const fract = (x) => x - Math.floor(x);
export const remap = (x, a, b, c, d) => c + ((x - a) * (d - c)) / (b - a);
export const smoothstep = (e0, e1, x) => {
    const t = clamp((x - e0) / (e1 - e0));
    return t * t * (3 - 2 * t);
};
export const angleDelta = (a, b) => ((((b - a) % 360) + 540) % 360) - 180;
export const mixAngle = (a, b, t) => a + angleDelta(a, b) * t;
