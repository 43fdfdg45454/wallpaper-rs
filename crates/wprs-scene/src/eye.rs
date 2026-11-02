pub type Mat4 = [f32; 16];

pub const IDENTITY4: Mat4 = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eye {
    pub at: [f32; 3],
    pub center: [f32; 3],
    pub up: [f32; 3],
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub aspect: f32,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn unit3(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt();
    if l > 1e-12 { v.map(|x| x / l) } else { [0.0, 0.0, 1.0] }
}

pub fn mul4(a: &Mat4, b: &Mat4) -> Mat4 {
    std::array::from_fn(|k| (0..4).map(|i| a[i * 4 + k % 4] * b[(k / 4) * 4 + i]).sum())
}

pub fn apply4(m: &Mat4, [x, y, z]: [f32; 3], w: f32) -> [f32; 4] {
    std::array::from_fn(|r| m[r] * x + m[4 + r] * y + m[8 + r] * z + m[12 + r] * w)
}

pub fn placement(at: [f32; 3], degrees: [f32; 3], scale: [f32; 3]) -> Mat4 {
    let [x, y, z] = degrees.map(f32::to_radians);
    let (sx, cx, sy, cy, sz, cz) = (x.sin(), x.cos(), y.sin(), y.cos(), z.sin(), z.cos());
    let r = [
        [cy * cz, cy * sz, -sy],
        [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy],
        [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy],
    ];
    let c = |i: usize, k: usize| r[i][k] * scale[i];
    let (x, y, z) = ([c(0, 0), c(0, 1), c(0, 2)], [c(1, 0), c(1, 1), c(1, 2)], [c(2, 0), c(2, 1), c(2, 2)]);
    [x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, at[0], at[1], at[2], 1.0]
}

impl Eye {
    pub fn forward(&self) -> [f32; 3] {
        unit3(sub(self.center, self.at))
    }

    pub fn basis(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let f = self.forward();
        let mut r = cross(f, unit3(self.up));
        if dot(r, r) < 1e-10 {
            r = cross(f, if f[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] });
        }
        let r = unit3(r);
        (r, cross(r, f), f)
    }

    pub fn view(&self) -> Mat4 {
        let (r, u, f) = self.basis();
        let e = self.at;
        let last = [-dot(r, e), -dot(u, e), dot(f, e), 1.0];
        [
            r[0], u[0], -f[0], 0.0, r[1], u[1], -f[1], 0.0, r[2], u[2], -f[2], 0.0, last[0], last[1],
            last[2], last[3],
        ]
    }

    pub fn projection(&self) -> Mat4 {
        let t = 1.0 / (self.fov.clamp(1.0, 179.0).to_radians() * 0.5).tan();
        let (n, f) = (self.near.max(1e-4), self.far.max(self.near * 1.0001));
        let k = n / (f - n);
        [t / self.aspect.max(1e-6), 0.0, 0.0, 0.0, 0.0, t, 0.0, 0.0, 0.0, 0.0, k, -1.0, 0.0, 0.0, f * k, 0.0]
    }

    pub fn clip(&self) -> Mat4 {
        mul4(&self.projection(), &self.view())
    }

    pub fn distance(&self) -> f32 {
        dot(sub(self.center, self.at), sub(self.center, self.at)).sqrt().max(1e-4)
    }

    pub fn depth(&self, p: [f32; 3]) -> f32 {
        dot(sub(p, self.at), self.forward())
    }

    pub fn unit(&self, height: f32) -> f32 {
        2.0 * self.distance() * (self.fov.clamp(1.0, 179.0).to_radians() * 0.5).tan() / height.max(1.0)
    }

    pub fn project(&self, p: [f32; 3], [w, h]: [f32; 2]) -> Option<[f32; 3]> {
        let c = apply4(&self.clip(), p, 1.0);
        (c[3] > 1e-6).then(|| [(c[0] / c[3] + 1.0) * 0.5 * w, (1.0 - c[1] / c[3]) * 0.5 * h, c[2] / c[3]])
    }

    pub fn place(&self, [x, y]: [f32; 2], z: f32, [w, h]: [f32; 2]) -> [f32; 3] {
        let (r, u, f) = self.basis();
        let k = self.unit(h);
        let (dx, dy) = ((x - w * 0.5) * k, (h * 0.5 - y) * k);
        std::array::from_fn(|i| self.center[i] + r[i] * dx + u[i] * dy - f[i] * z * k)
    }
}
