pub(super) type M3 = [[f32; 3]; 3];

pub(super) fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}

pub(super) fn apply(a: &M3, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| (0..3).map(|k| a[r][k] * v[k]).sum())
}

pub(super) fn flip(a: &M3) -> M3 {
    std::array::from_fn(|r| std::array::from_fn(|c| a[c][r]))
}

pub(super) fn of_mat4(m: &wprs_scene::Mat4) -> M3 {
    std::array::from_fn(|r| std::array::from_fn(|c| m[c * 4 + r]))
}

pub(super) fn of_mat(m: &wprs_scene::Mat) -> M3 {
    std::array::from_fn(|r| std::array::from_fn(|c| m[r * 4 + c]))
}

pub(super) fn pure(a: &M3) -> M3 {
    let len = |c: usize| (0..3).map(|r| a[r][c] * a[r][c]).sum::<f32>().sqrt().max(1e-9);
    let l = [len(0), len(1), len(2)];
    std::array::from_fn(|r| std::array::from_fn(|c| a[r][c] / l[c]))
}

pub(super) fn invert(a: &M3) -> Option<M3> {
    let cof = |r: usize, c: usize| {
        let (r1, r2, c1, c2) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        a[r1][c1] * a[r2][c2] - a[r1][c2] * a[r2][c1]
    };
    let det: f32 = (0..3).map(|c| a[0][c] * cof(0, c)).sum();
    (det.abs() > 1e-12).then(|| std::array::from_fn(|r| std::array::from_fn(|c| cof(c, r) / det)))
}

pub(super) fn around(axis: [f32; 3], degrees: f32) -> M3 {
    let [x, y, z] = wprs_scene::unit3(axis);
    let (s, c) = degrees.to_radians().sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

pub(super) fn euler(d: [f32; 3]) -> M3 {
    mul(&around([0.0, 0.0, 1.0], d[2]), &mul(&around([0.0, 1.0, 0.0], d[1]), &around([1.0, 0.0, 0.0], d[0])))
}

pub(super) fn angles(r: &M3) -> [f32; 3] {
    let y = (-r[2][0]).clamp(-1.0, 1.0).asin();
    let (x, z) = match r[2][0].abs() > 0.9999 {
        true => ((-r[1][2]).atan2(r[1][1]), 0.0),
        false => (r[2][1].atan2(r[2][2]), r[1][0].atan2(r[0][0])),
    };
    [x, y, z].map(|v| (v.to_degrees() * 100.0).round() / 100.0)
}

pub(super) fn now(p: &wprs_scene::Placed) -> serde_json::Value {
    let m = of_mat4(&p.matrix);
    let size = |c: usize| (0..3).map(|r| m[r][c] * m[r][c]).sum::<f32>().sqrt();
    let scale = [size(0), size(1), size(2)];
    let travel = p.pose.map_or([0.0; 3], |q| q.travel);
    let moved = apply(&m, travel);
    let at: [f32; 3] = std::array::from_fn(|i| p.matrix[12 + i] - moved[i]);
    let r = |v: [f32; 3]| v.map(|x| (x * 1000.0).round() / 1000.0);
    serde_json::json!({ "position": r(at), "rotation": angles(&pure(&m)), "scale": r(scale) })
}
