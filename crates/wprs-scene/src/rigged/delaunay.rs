use super::cut::Mask;
use std::collections::HashMap;

type P = [f32; 2];

pub(super) fn outline(mask: &Mask, fine: f32, spacing: f32) -> Vec<P> {
    let [w, h] = mask.size.map(|v| v as f32);
    let (gx, gy) = ((w / fine).ceil() as i32, (h / fine).ceil() as i32);
    let filled = |i: i32, j: i32| {
        (0..gx).contains(&i) && (0..gy).contains(&j) && {
            let (x, y) = (i as f32 * fine, j as f32 * fine);
            mask.solid(x, y, x + fine, y + fine)
        }
    };
    let mut next: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
    for j in 0..gy {
        for i in 0..gx {
            if !filled(i, j) {
                continue;
            }
            let sides = [
                ((0, -1), (i, j), (i + 1, j)),
                ((1, 0), (i + 1, j), (i + 1, j + 1)),
                ((0, 1), (i + 1, j + 1), (i, j + 1)),
                ((-1, 0), (i, j + 1), (i, j)),
            ];
            for ((dx, dy), a, b) in sides {
                if !filled(i + dx, j + dy) {
                    next.entry(a).or_default().push(b);
                }
            }
        }
    }
    let mut best: Vec<(i32, i32)> = Vec::new();
    let starts: Vec<(i32, i32)> = next.keys().copied().collect();
    for start in starts {
        let mut ring = Vec::new();
        let mut at = start;
        while let Some(to) = next.get_mut(&at).and_then(Vec::pop) {
            ring.push(at);
            at = to;
        }
        if ring.len() > best.len() {
            best = ring;
        }
    }
    let corner = |(i, j): (i32, i32)| [(i as f32 * fine).min(w), (j as f32 * fine).min(h)];
    resample(&best.into_iter().map(corner).collect::<Vec<_>>(), spacing)
}

fn resample(ring: &[P], spacing: f32) -> Vec<P> {
    let mut out = Vec::new();
    let mut left = 0.0;
    for k in 0..ring.len() {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let long = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let mut t = left;
        while t < long {
            let f = t / long;
            out.push([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]);
            t += spacing;
        }
        left = t - long;
    }
    out
}

pub(super) fn inside(ring: &[P], p: P) -> bool {
    let mut odd = false;
    for k in 0..ring.len() {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

pub(super) fn near(ring: &[P], p: P) -> f32 {
    let segment = |a: P, b: P| {
        let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
        let long = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if long > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1]) / long).clamp(0.0, 1.0) } else { 0.0 };
        ((ap[0] - ab[0] * t).powi(2) + (ap[1] - ab[1] * t).powi(2)).sqrt()
    };
    (0..ring.len()).map(|k| segment(ring[k], ring[(k + 1) % ring.len()])).fold(f32::MAX, f32::min)
}

fn circle(p: &[P], [a, b, c]: [usize; 3]) -> (P, f32) {
    let ([ax, ay], [bx, by], [cx, cy]) = (p[a], p[b], p[c]);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    let sq = |x: f32, y: f32| x * x + y * y;
    let ux = (sq(ax, ay) * (by - cy) + sq(bx, by) * (cy - ay) + sq(cx, cy) * (ay - by)) / d;
    let uy = (sq(ax, ay) * (cx - bx) + sq(bx, by) * (ax - cx) + sq(cx, cy) * (bx - ax)) / d;
    ([ux, uy], sq(ax - ux, ay - uy))
}

pub(super) fn triangulate(points: &[P]) -> Vec<[u32; 3]> {
    let (lo, hi) = points.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
    });
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0) * 20.0;
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let n = points.len();
    let mut p = points.to_vec();
    p.extend([[mid[0] - span, mid[1] - span], [mid[0] + span, mid[1] - span], [mid[0], mid[1] + span]]);
    let mut tris = vec![([n, n + 1, n + 2], circle(&p, [n, n + 1, n + 2]))];
    for k in 0..n {
        let q = p[k];
        let bad = |(_, (c, r)): &([usize; 3], (P, f32))| (q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2) < *r;
        let (broken, kept): (Vec<_>, Vec<_>) = tris.into_iter().partition(bad);
        let edges: Vec<(usize, usize)> =
            broken.iter().flat_map(|(t, _)| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]).collect();
        let shared =
            |e: &(usize, usize)| edges.iter().filter(|f| (f.0, f.1) == (e.1, e.0) || *f == e).count() > 1;
        tris = kept;
        for e in edges.iter().filter(|e| !shared(e)) {
            let t = [e.0, e.1, k];
            tris.push((t, circle(&p, t)));
        }
    }
    tris.into_iter().filter(|(t, _)| t.iter().all(|i| *i < n)).map(|(t, _)| t.map(|i| i as u32)).collect()
}
