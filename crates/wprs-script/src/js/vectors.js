import { KEYS, clamp, degrees, list, radians } from "wprs:scalars";

export class Vector {
    constructor(...c) {
        for (let i = 0; i < this.constructor.size; i++) this[KEYS[i]] = Number(c[i] ?? 0);
    }
    static from(v) {
        if (v instanceof this) return v;
        const l = list(v);
        return l === null ? new this(...Array(this.size).fill(Number(v))) : new this(...l);
    }
    toArray() {
        return KEYS.slice(0, this.constructor.size).map((k) => this[k]);
    }
    toJSON() {
        return this.toArray();
    }
    toString() {
        return `${this.constructor.name}(${this.toArray().join(", ")})`;
    }
    clone() {
        return new this.constructor(...this.toArray());
    }
    map(f) {
        return new this.constructor(...this.toArray().map(f));
    }
    zip(o, f) {
        const b = this.constructor.from(o).toArray();
        return new this.constructor(...this.toArray().map((a, i) => f(a, b[i])));
    }
    add(o) {
        return this.zip(o, (a, b) => a + b);
    }
    sub(o) {
        return this.zip(o, (a, b) => a - b);
    }
    mul(o) {
        return this.zip(o, (a, b) => a * b);
    }
    div(o) {
        return this.zip(o, (a, b) => a / b);
    }
    min(o) {
        return this.zip(o, Math.min);
    }
    max(o) {
        return this.zip(o, Math.max);
    }
    clamp(lo, hi) {
        return this.max(lo).min(hi);
    }
    scale(s) {
        return this.map((a) => a * s);
    }
    negate() {
        return this.map((a) => -a);
    }
    abs() {
        return this.map(Math.abs);
    }
    floor() {
        return this.map(Math.floor);
    }
    ceil() {
        return this.map(Math.ceil);
    }
    round() {
        return this.map(Math.round);
    }
    dot(o) {
        const b = this.constructor.from(o).toArray();
        return this.toArray().reduce((s, a, i) => s + a * b[i], 0);
    }
    lengthSquared() {
        return this.dot(this);
    }
    length() {
        return Math.sqrt(this.lengthSquared());
    }
    normalize() {
        const l = this.length();
        return l > 0 ? this.scale(1 / l) : this.clone();
    }
    distance(o) {
        return this.sub(o).length();
    }
    mix(o, t) {
        return this.zip(o, (a, b) => a + (b - a) * t);
    }
    lerp(o, t) {
        return this.mix(o, t);
    }
    reflect(n) {
        const u = this.constructor.from(n).normalize();
        return this.sub(u.scale(2 * this.dot(u)));
    }
    project(o) {
        const b = this.constructor.from(o);
        const l = b.lengthSquared();
        return b.scale(l > 0 ? this.dot(b) / l : 0);
    }
    equals(o, eps = 1e-6) {
        const b = this.constructor.from(o).toArray();
        return this.toArray().every((a, i) => Math.abs(a - b[i]) <= eps);
    }
    isZero(eps = 1e-6) {
        return this.toArray().every((a) => Math.abs(a) <= eps);
    }
}

export class Vec2 extends Vector {
    static size = 2;
    cross(o) {
        const b = Vec2.from(o);
        return this.x * b.y - this.y * b.x;
    }
    angle() {
        return degrees(Math.atan2(this.y, this.x));
    }
    rotate(deg) {
        const [c, s] = [Math.cos(radians(deg)), Math.sin(radians(deg))];
        return new Vec2(this.x * c - this.y * s, this.x * s + this.y * c);
    }
    perpendicular() {
        return new Vec2(-this.y, this.x);
    }
}

export class Vec3 extends Vector {
    static size = 3;
    cross(o) {
        const b = Vec3.from(o);
        return new Vec3(this.y * b.z - this.z * b.y, this.z * b.x - this.x * b.z, this.x * b.y - this.y * b.x);
    }
}

export class Vec4 extends Vector {
    static size = 4;
}
