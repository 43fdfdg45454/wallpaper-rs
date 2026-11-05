pub const PALETTE_SIDE: u32 = 64;
pub const FALLBACK: [[f32; 4]; 5] = [
    [0.16, 0.16, 0.18, 1.0],
    [0.36, 0.36, 0.4, 1.0],
    [0.6, 0.6, 0.64, 1.0],
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0],
];
const ROUNDS: usize = 12;
const READABLE: f32 = 4.5;

type Lab = [f32; 3];

fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn encoded(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn lab([r, g, b]: [f32; 3]) -> Lab {
    let [r, g, b] = [r, g, b].map(linear);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn rgb([big_l, a, b]: Lab) -> [f32; 4] {
    let l = (big_l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (big_l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (big_l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let r = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
    let g = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s;
    let b = -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s;
    [encoded(r), encoded(g), encoded(b), 1.0]
}

fn far(a: Lab, b: Lab) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

fn nearest(centres: &[Lab], p: Lab) -> usize {
    let mut best = (0, f32::INFINITY);
    for (i, c) in centres.iter().enumerate() {
        let d = far(*c, p);
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

fn seeds(points: &[Lab]) -> Vec<Lab> {
    let n = points.len() as f32;
    let mean = points.iter().fold([0.0; 3], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n]);
    let mut centres = vec![points[nearest(points, mean)]];
    while centres.len() < 3 {
        let gap = |p: &Lab| centres.iter().map(|c| far(*c, *p)).fold(f32::INFINITY, f32::min);
        let next = points.iter().copied().max_by(|a, b| gap(a).total_cmp(&gap(b)));
        centres.push(next.unwrap_or(mean));
    }
    centres
}

pub fn luminance(c: [f32; 4]) -> f32 {
    let [r, g, b] = [c[0], c[1], c[2]].map(linear);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

pub fn contrast(a: [f32; 4], b: [f32; 4]) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn readable(primary: [f32; 4], others: [[f32; 4]; 2]) -> ([f32; 4], [f32; 4]) {
    let (white, black) = ([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
    let plain = if contrast(primary, white) >= contrast(primary, black) { white } else { black };
    let best = others.into_iter().max_by(|a, b| contrast(primary, *a).total_cmp(&contrast(primary, *b)));
    let text = best.filter(|c| contrast(primary, *c) >= READABLE).unwrap_or(plain);
    (text, plain)
}

pub fn palette(rgba: &[u8]) -> Option<[[f32; 4]; 5]> {
    let seen = rgba.as_chunks::<4>().0.iter().filter(|p| p[3] >= 128);
    let points: Vec<Lab> = seen.map(|p| lab([p[0], p[1], p[2]].map(|v| f32::from(v) / 255.0))).collect();
    if points.is_empty() {
        return None;
    }
    let mut centres = seeds(&points);
    let mut counts = [0usize; 3];
    for _ in 0..ROUNDS {
        let mut sums = [[0.0f32; 3]; 3];
        counts = [0; 3];
        for p in &points {
            let i = nearest(&centres, *p);
            counts[i] += 1;
            (0..3).for_each(|k| sums[i][k] += p[k]);
        }
        let moved: Vec<Lab> = (0..3)
            .map(|i| match counts[i] {
                0 => centres[i],
                n => sums[i].map(|s| s / n as f32),
            })
            .collect();
        let still = moved.iter().zip(&centres).all(|(a, b)| far(*a, *b) < 1e-8);
        centres = moved;
        if still {
            break;
        }
    }
    let mut order = [0, 1, 2];
    order.sort_by(|a, b| counts[*b].cmp(&counts[*a]).then(a.cmp(b)));
    let [primary, secondary, tertiary] = order.map(|i| rgb(centres[i]));
    let (text, plain) = readable(primary, [secondary, tertiary]);
    Some([primary, secondary, tertiary, text, plain])
}

pub fn hex(c: [f32; 4]) -> String {
    let [r, g, b, _] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}
