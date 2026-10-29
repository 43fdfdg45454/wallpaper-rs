use crate::Color;
use std::f32::consts::{PI, TAU};

fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn encoded(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn lch(Color([r, g, b, _]): Color) -> [f32; 3] {
    let [r, g, b] = [r, g, b].map(linear);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let lab = [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ];
    [lab[0], lab[1].hypot(lab[2]), lab[2].atan2(lab[1])]
}

fn rgb([big_l, c, h]: [f32; 3], alpha: f32) -> Color {
    let (a, b) = (c * h.cos(), c * h.sin());
    let l = (big_l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (big_l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (big_l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let r = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
    let g = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s;
    let b = -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s;
    Color([encoded(r), encoded(g), encoded(b), alpha])
}

pub(crate) fn mix(from: Color, to: Color, f: f32) -> Color {
    if f <= 0.0 || from == to {
        return from;
    }
    if f >= 1.0 {
        return to;
    }
    let ([l0, c0, h0], [l1, c1, h1]) = (lch(from), lch(to));
    let grey = 1e-4;
    let (h0, h1) = match (c0 < grey, c1 < grey) {
        (true, false) => (h1, h1),
        (false, true) => (h0, h0),
        _ => (h0, h1),
    };
    let turn = (h1 - h0 + PI).rem_euclid(TAU) - PI;
    let at = |a: f32, b: f32| a + (b - a) * f;
    rgb([at(l0, l1), at(c0, c1), h0 + turn * f], at(from.0[3], to.0[3]))
}
