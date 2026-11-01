pub(super) fn homography([p0, p1, p2, p3]: [[f32; 2]; 4]) -> [[f32; 4]; 3] {
    let (dx, dy) = ([p1[0] - p2[0], p3[0] - p2[0]], [p1[1] - p2[1], p3[1] - p2[1]]);
    let sum = [0, 1].map(|i| p0[i] - p1[i] + p2[i] - p3[i]);
    let den = dx[0] * dy[1] - dx[1] * dy[0];
    let (g, h) = match den.abs() > 1e-9 {
        true => ((sum[0] * dy[1] - dx[1] * sum[1]) / den, (dx[0] * sum[1] - sum[0] * dy[0]) / den),
        false => (0.0, 0.0),
    };
    let [a, d] = [0, 1].map(|i| p1[i] - p0[i] + g * p1[i]);
    let [b, e] = [0, 1].map(|i| p3[i] - p0[i] + h * p3[i]);
    let [c, f] = p0;
    [
        [e - f * h, c * h - b, b * f - c * e, 0.0],
        [f * g - d, a - c * g, c * d - a * f, 0.0],
        [d * h - e * g, b * g - a * h, a * e - b * d, 0.0],
    ]
}
