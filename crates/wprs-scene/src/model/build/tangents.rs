use crate::rigged::math::Mat;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(v: [f32; 3], or: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt();
    if l > 1e-12 { v.map(|x| x / l) } else { or }
}

pub(super) fn along(m: &Mat, t: &[f32], flipped: bool) -> [f32; 4] {
    let d: [f32; 3] = std::array::from_fn(|r| m[r * 4] * t[0] + m[r * 4 + 1] * t[1] + m[r * 4 + 2] * t[2]);
    let [x, y, z] = unit(d, [1.0, 0.0, 0.0]);
    [x, y, z, if flipped { -t[3] } else { t[3] }]
}

pub(super) fn smooth(at: &[[f32; 3]], list: &[u32]) -> Vec<[f32; 3]> {
    let mut sum = vec![[0.0f32; 3]; at.len()];
    for t in list.as_chunks::<3>().0 {
        let [a, b, c] = t.map(|i| i as usize);
        let n = cross(sub(at[b], at[a]), sub(at[c], at[a]));
        [a, b, c].into_iter().for_each(|i| (0..3).for_each(|k| sum[i][k] += n[k]));
    }
    sum.into_iter().map(|n| unit(n, [0.0, 0.0, 1.0])).collect()
}

pub(super) fn tangents(at: &[[f32; 3]], normals: &[[f32; 3]], uv: &[f32], list: &[u32]) -> Vec<[f32; 4]> {
    let (mut tan, mut bit) = (vec![[0.0f32; 3]; at.len()], vec![[0.0f32; 3]; at.len()]);
    for t in list.as_chunks::<3>().0 {
        let [a, b, c] = t.map(|i| i as usize);
        let (e1, e2) = (sub(at[b], at[a]), sub(at[c], at[a]));
        let (du1, dv1) = (uv[b * 2] - uv[a * 2], uv[b * 2 + 1] - uv[a * 2 + 1]);
        let (du2, dv2) = (uv[c * 2] - uv[a * 2], uv[c * 2 + 1] - uv[a * 2 + 1]);
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-12 {
            continue;
        }
        let r = 1.0 / det;
        let s: [f32; 3] = std::array::from_fn(|k| (e1[k] * dv2 - e2[k] * dv1) * r);
        let u: [f32; 3] = std::array::from_fn(|k| (e2[k] * du1 - e1[k] * du2) * r);
        for i in [a, b, c] {
            (0..3).for_each(|k| (tan[i][k], bit[i][k]) = (tan[i][k] + s[k], bit[i][k] + u[k]));
        }
    }
    let side = |n: [f32; 3]| if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    (0..at.len())
        .map(|i| {
            let n = normals[i];
            let t = sub(tan[i], n.map(|x| x * dot(n, tan[i])));
            let [x, y, z] = unit(t, unit(cross(n, side(n)), [1.0, 0.0, 0.0]));
            let w = if dot(cross(n, [x, y, z]), bit[i]) < 0.0 { -1.0 } else { 1.0 };
            [x, y, z, w]
        })
        .collect()
}
