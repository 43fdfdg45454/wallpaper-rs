pub fn spline(points: &[[f32; 2]]) -> impl Fn(f32) -> f32 + '_ {
    let n = points.len();
    let d: Vec<f32> =
        points.windows(2).map(|p| (p[1][1] - p[0][1]) / (p[1][0] - p[0][0]).max(1e-6)).collect();
    let mut m: Vec<f32> = (0..n)
        .map(|i| match i {
            0 => d.first().copied().unwrap_or(1.0),
            i if i == n - 1 => d[i - 1],
            i if d[i - 1] * d[i] <= 0.0 => 0.0,
            i => (d[i - 1] + d[i]) / 2.0,
        })
        .collect();
    for (i, slope) in d.iter().enumerate() {
        let (a, b) = (m[i] / slope, m[i + 1] / slope);
        if *slope == 0.0 {
            (m[i], m[i + 1]) = (0.0, 0.0);
        } else if a * a + b * b > 9.0 {
            let t = 3.0 / (a * a + b * b).sqrt();
            (m[i], m[i + 1]) = (t * a * slope, t * b * slope);
        }
    }
    move |x| {
        if n < 2 {
            return x;
        }
        let x = x.clamp(points[0][0], points[n - 1][0]);
        let i = points.windows(2).position(|p| x <= p[1][0]).unwrap_or(n - 2);
        let ([x0, y0], [x1, y1]) = (points[i], points[i + 1]);
        let (h, t) = (x1 - x0, (x - x0) / (x1 - x0).max(1e-6));
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * y0
            + (t3 - 2.0 * t2 + t) * h * m[i]
            + (-2.0 * t3 + 3.0 * t2) * y1
            + (t3 - t2) * h * m[i + 1]
    }
}
