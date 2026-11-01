export { degrees, radians, clamp, step, fract, remap, smoothstep, angleDelta, mixAngle } from "wprs:scalars";
export { Vec2, Vec3, Vec4 } from "wprs:vectors";
export { Mat3, Mat4 } from "wprs:matrices";
import { list, degrees, radians, fract } from "wprs:scalars";
import { Vector, Vec2 } from "wprs:vectors";

export const angleVector = (deg) => new Vec2(Math.cos(radians(deg)), Math.sin(radians(deg)));
export const vectorAngle = (v) => {
    const [x, y] = list(v);
    return degrees(Math.atan2(y, x));
};
export const lookAt = (from, to) => {
    const [[ax, ay], [bx, by]] = [list(from), list(to)];
    return degrees(Math.atan2(by - ay, bx - ax));
};

export function mix(a, b, t) {
    if (typeof a === "number") return a + (b - a) * t;
    if (a instanceof Vector) return a.mix(b, t);
    const [x, y] = [list(a), list(b)];
    return x.map((v, i) => v + (y[i] - v) * t);
}

export function rgbToHsv(c) {
    const [r, g, b, ...rest] = list(c);
    const [hi, lo] = [Math.max(r, g, b), Math.min(r, g, b)];
    const d = hi - lo;
    let h = 0;
    if (d > 0) h = hi === r ? ((g - b) / d + 6) % 6 : hi === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return [h / 6, hi > 0 ? d / hi : 0, hi, ...rest];
}

export function hsvToRgb(c) {
    const [h, s, v, ...rest] = list(c);
    const f = (n) => {
        const k = (n + fract(h) * 6) % 6;
        return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
    };
    return [f(5), f(3), f(1), ...rest];
}
