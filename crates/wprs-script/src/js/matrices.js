import { clamp, degrees, list, radians } from "wprs:scalars";
import { Vec2, Vec3 } from "wprs:vectors";

const identity = (n) => Array.from({ length: n * n }, (_, i) => (i % (n + 1) === 0 ? 1 : 0));

class Matrix {
    constructor(values) {
        const n = this.constructor.n;
        this.m = values === undefined ? identity(n) : Array.from(values, Number);
        if (this.m.length !== n * n || !this.m.every(Number.isFinite)) throw new TypeError(`a matrix takes ${n * n} numbers`);
    }
    static identity() {
        return new this();
    }
    static from(v) {
        return v instanceof this ? v : new this(v?.toArray ? v.toArray() : v);
    }
    get(row, col) {
        return this.m[col * this.constructor.n + row];
    }
    toArray() {
        return [...this.m];
    }
    toJSON() {
        return this.toArray();
    }
    clone() {
        return new this.constructor(this.m);
    }
    equals(o, eps = 1e-6) {
        const b = this.constructor.from(o).m;
        return this.m.every((a, i) => Math.abs(a - b[i]) <= eps);
    }
    multiply(o) {
        const [n, a, b] = [this.constructor.n, this.m, this.constructor.from(o).m];
        const out = Array.from({ length: n * n }, (_, i) => {
            const [c, r] = [Math.floor(i / n), i % n];
            let s = 0;
            for (let k = 0; k < n; k++) s += a[k * n + r] * b[c * n + k];
            return s;
        });
        return new this.constructor(out);
    }
    transpose() {
        const n = this.constructor.n;
        return new this.constructor(this.m.map((_, i) => this.m[(i % n) * n + Math.floor(i / n)]));
    }
    solve() {
        const n = this.constructor.n;
        const rows = Array.from({ length: n }, (_, r) => [...Array.from({ length: n }, (_, c) => this.get(r, c)), ...identity(n).slice(r * n, r * n + n)]);
        let det = 1;
        for (let c = 0; c < n; c++) {
            let p = c;
            for (let r = c + 1; r < n; r++) if (Math.abs(rows[r][c]) > Math.abs(rows[p][c])) p = r;
            if (Math.abs(rows[p][c]) < 1e-12) return { det: 0, inverse: null };
            if (p !== c) [rows[p], rows[c], det] = [rows[c], rows[p], -det];
            const pivot = rows[c][c];
            det *= pivot;
            rows[c] = rows[c].map((v) => v / pivot);
            for (let r = 0; r < n; r++) {
                if (r === c) continue;
                const k = rows[r][c];
                rows[r] = rows[r].map((v, i) => v - k * rows[c][i]);
            }
        }
        const inverse = Array.from({ length: n * n }, (_, i) => rows[i % n][n + Math.floor(i / n)]);
        return { det, inverse: new this.constructor(inverse) };
    }
    determinant() {
        return this.solve().det;
    }
    invert() {
        return this.solve().inverse;
    }
}

export class Mat3 extends Matrix {
    static n = 3;
    static translation(x, y) {
        return new Mat3([1, 0, 0, 0, 1, 0, x, y, 1]);
    }
    static rotation(deg) {
        const [c, s] = [Math.cos(radians(deg)), Math.sin(radians(deg))];
        return new Mat3([c, s, 0, -s, c, 0, 0, 0, 1]);
    }
    static scaling(x, y = x) {
        return new Mat3([x, 0, 0, 0, y, 0, 0, 0, 1]);
    }
    transformPoint(v) {
        const [x, y] = list(v);
        const m = this.m;
        return new Vec2(m[0] * x + m[3] * y + m[6], m[1] * x + m[4] * y + m[7]);
    }
    transformVector(v) {
        const [x, y] = list(v);
        const m = this.m;
        return new Vec2(m[0] * x + m[3] * y, m[1] * x + m[4] * y);
    }
    decompose() {
        const m = this.m;
        const sx = Math.hypot(m[0], m[1]);
        const sy = (m[0] * m[4] - m[1] * m[3]) / (sx || 1);
        return { translation: new Vec2(m[6], m[7]), rotation: degrees(Math.atan2(m[1], m[0])), scale: new Vec2(sx, sy) };
    }
}

export class Mat4 extends Matrix {
    static n = 4;
    static translation(x, y, z = 0) {
        return new Mat4([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]);
    }
    static scaling(x, y = x, z = x) {
        return new Mat4([x, 0, 0, 0, 0, y, 0, 0, 0, 0, z, 0, 0, 0, 0, 1]);
    }
    static rotationX(deg) {
        const [c, s] = [Math.cos(radians(deg)), Math.sin(radians(deg))];
        return new Mat4([1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]);
    }
    static rotationY(deg) {
        const [c, s] = [Math.cos(radians(deg)), Math.sin(radians(deg))];
        return new Mat4([c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]);
    }
    static rotationZ(deg) {
        const [c, s] = [Math.cos(radians(deg)), Math.sin(radians(deg))];
        return new Mat4([c, s, 0, 0, -s, c, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    }
    static fromEuler(x, y, z) {
        return Mat4.rotationZ(z).multiply(Mat4.rotationY(y)).multiply(Mat4.rotationX(x));
    }
    static perspective(fov, aspect, near, far) {
        const f = 1 / Math.tan(radians(fov) / 2);
        const k = 1 / (near - far);
        return new Mat4([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * k, -1, 0, 0, 2 * far * near * k, 0]);
    }
    static lookAt(eye, target, up = [0, 1, 0]) {
        const e = Vec3.from(eye);
        const z = e.sub(target).normalize();
        const x = Vec3.from(up).cross(z).normalize();
        const y = z.cross(x);
        return new Mat4([x.x, y.x, z.x, 0, x.y, y.y, z.y, 0, x.z, y.z, z.z, 0, -x.dot(e), -y.dot(e), -z.dot(e), 1]);
    }
    transformPoint(v) {
        const [x, y, z = 0] = list(v);
        const m = this.m;
        const w = m[3] * x + m[7] * y + m[11] * z + m[15] || 1;
        const at = (r) => (m[r] * x + m[4 + r] * y + m[8 + r] * z + m[12 + r]) / w;
        return new Vec3(at(0), at(1), at(2));
    }
    transformVector(v) {
        const [x, y, z = 0] = list(v);
        const m = this.m;
        const at = (r) => m[r] * x + m[4 + r] * y + m[8 + r] * z;
        return new Vec3(at(0), at(1), at(2));
    }
    decompose() {
        const m = this.m;
        const scale = new Vec3(Math.hypot(m[0], m[1], m[2]), Math.hypot(m[4], m[5], m[6]), Math.hypot(m[8], m[9], m[10]));
        const r = (i, s) => m[i] / (s || 1);
        const [r20, r21, r22, r10, r00] = [r(2, scale.x), r(6, scale.y), r(10, scale.z), r(1, scale.x), r(0, scale.x)];
        const y = degrees(Math.asin(clamp(-r20, -1, 1)));
        const rotation = new Vec3(degrees(Math.atan2(r21, r22)), y, degrees(Math.atan2(r10, r00)));
        return { translation: new Vec3(m[12], m[13], m[14]), scale, rotation };
    }
}
