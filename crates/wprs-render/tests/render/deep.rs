use super::renderer;
use wprs_render::{Coding, DeepPlanes, GAMUT, KNEE, PEAK_NITS, Planes, REFERENCE_NITS, deep_rows};

fn pq(e: f32) -> f32 {
    let p = e.max(0.0).powf(1.0 / 78.84375);
    10000.0 * ((p - 0.8359375).max(0.0) / (18.851_563 - 18.6875 * p)).powf(1.0 / 0.159_301_76)
}

fn hlg(e: [f32; 3]) -> [f32; 3] {
    let lin = e.map(|e| {
        if e <= 0.5 { e * e / 3.0 } else { (((e - 0.559_910_7) / 0.178_832_77).exp() + 0.284_668_92) / 12.0 }
    });
    let y = 0.2627 * lin[0] + 0.6780 * lin[1] + 0.0593 * lin[2];
    lin.map(|l| 1000.0 * l * y.max(1e-6).powf(0.2))
}

fn reference(codes: [u16; 3], hlg_: bool) -> [u8; 3] {
    let ycc = [codes[0], codes[1], codes[2]].map(|c| f32::from(c) / 1023.0);
    let e = deep_rows(false).map(|r| (r[0] * ycc[0] + r[1] * ycc[1] + r[2] * ycc[2] + r[3]).clamp(0.0, 1.0));
    let nits = if hlg_ { hlg(e) } else { e.map(pq) };
    let light = nits.map(|n| n / REFERENCE_NITS);
    let c = GAMUT.map(|m| (m[0] * light[0] + m[1] * light[1] + m[2] * light[2]).max(0.0));
    let l = c[0].max(c[1]).max(c[2]);
    let (k, wx) = (KNEE, (PEAK_NITS / REFERENCE_NITS - KNEE) / (1.0 - KNEE));
    let x = (l - k).max(0.0) / (1.0 - k);
    let mapped = if l > k { k + (1.0 - k) * x * (1.0 + x / (wx * wx)) / (1.0 + x) } else { l };
    c.map(|v| {
        let v = (v * mapped / l.max(1e-6)).clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round() as u8
    })
}

fn bytes(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn hdr_frames_are_mapped_to_standard_colors_on_the_gpu() {
    let Some(r) = renderer() else { return };
    let swatches: [[u16; 3]; 4] = [[509, 512, 512], [700, 512, 512], [400, 420, 640], [880, 600, 470]];
    for hlg_ in [false, true] {
        let mut slot = None;
        for codes in swatches {
            let (luma, cb, cr) = (bytes(&[codes[0]; 16]), bytes(&[codes[1]; 4]), bytes(&[codes[2]; 4]));
            let planes = [(luma.as_slice(), 8), (cb.as_slice(), 4), (cr.as_slice(), 4)];
            let image = r
                .convert_deep(
                    &mut slot,
                    &DeepPlanes { size: (4, 4), planes, hlg: hlg_, full: false, headroom: 1.0 },
                )
                .unwrap();
            let mut target = r.create_target(4, 4).unwrap();
            let layer = super::layer(Some(image), wprs_core::Fit::Stretch, [0; 3], 1.0);
            let mut pixels = Vec::new();
            r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
            let (got, want) = (super::rgb(&pixels, 4, 1, 1), reference(codes, hlg_));
            assert!(
                got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 3),
                "{codes:?} hlg {hlg_}: {got:?} not {want:?}"
            );
        }
    }
    let white = reference([940, 512, 512], false);
    assert!(white.iter().all(|c| *c == 255) && reference([509, 512, 512], false)[0] > 150);
    let short = vec![0u8; 8];
    let planes = [(short.as_slice(), 8), (short.as_slice(), 4), (short.as_slice(), 4)];
    assert!(
        r.convert_deep(
            &mut None,
            &DeepPlanes { size: (4, 4), planes, hlg: false, full: false, headroom: 1.0 }
        )
        .is_err()
    );
}

#[test]
fn eight_bit_hdr_frames_are_mapped_like_ten_bit_ones() {
    let Some(r) = renderer() else { return };
    let swatches: [[u8; 3]; 3] = [[127, 128, 128], [175, 128, 128], [100, 105, 160]];
    for coding in [Coding::Pq, Coding::Hlg] {
        let (mut slot, mut plain) = (None, None);
        for codes in swatches {
            let (luma, chroma) = ([codes[0]; 16], [codes[1], codes[2]].repeat(4));
            let planes = Planes {
                size: (4, 4),
                luma: (&luma, 4),
                chroma: (&chroma, 4),
                bt709: true,
                full: false,
                coding,
                headroom: 1.0,
            };
            let shown = |image| {
                let mut target = r.create_target(4, 4).unwrap();
                let layer = super::layer(Some(image), wprs_core::Fit::Stretch, [0; 3], 1.0);
                let mut pixels = Vec::new();
                r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
                super::rgb(&pixels, 4, 1, 1)
            };
            let got = shown(r.convert_yuv(&mut slot, &planes).unwrap());
            let want = reference(codes.map(|c| u16::from(c) * 4), coding == Coding::Hlg);
            assert!(
                got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 3),
                "{codes:?} {coding:?}: {got:?} not {want:?}"
            );
            let sdr = shown(r.convert_yuv(&mut plain, &Planes { coding: Coding::Sdr, ..planes }).unwrap());
            assert_ne!(sdr, got, "{codes:?} {coding:?} read as standard colors");
        }
    }
}

#[test]
fn hdr_frames_keep_their_highlights_on_an_hdr_screen() {
    let Some(r) = renderer() else { return };
    let frame = |code: u8| ([code; 16], [128u8, 128].repeat(4));
    let lit = |code: u8, coding: Coding, headroom: f32| {
        let (luma, chroma) = frame(code);
        let planes = Planes {
            size: (4, 4),
            luma: (&luma, 4),
            chroma: (&chroma, 4),
            bt709: true,
            full: false,
            coding,
            headroom,
        };
        let mut slot = None;
        let image = r.convert_yuv(&mut slot, &planes).unwrap();
        (image.format(), r.read_light(image).ok().map(|light| light[5][0]))
    };
    let (format, bright) = lit(220, Coding::Pq, 4.0);
    assert_eq!(format, wgpu::TextureFormat::Rgba16Float);
    let bright = bright.unwrap();
    assert!(bright > 1.5 && bright <= 4.0, "a bright highlight shows at {bright}");
    let white = lit(143, Coding::Pq, 4.0).1.unwrap();
    assert!((white - 1.0).abs() < 0.12, "reference white shows at {white}");
    let hlg = lit(220, Coding::Hlg, 4.0).1.unwrap();
    assert!(hlg > 1.0 && hlg <= 4.0, "{hlg}");
    assert_eq!(
        lit(220, Coding::Pq, 1.0).0,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        "a standard screen keeps the mapped picture"
    );
    assert_eq!(
        lit(220, Coding::Sdr, 4.0).0,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        "standard videos stay as they are"
    );
    assert_eq!(lit(220, Coding::Pq, f32::NAN).0, wgpu::TextureFormat::Rgba8UnormSrgb);
}

#[test]
fn hdr_frames_keep_their_highlights_above_white_on_hdr_screens() {
    let Some(r) = renderer() else { return };
    let (luma, cb, cr) = (bytes(&[880; 16]), bytes(&[512; 4]), bytes(&[512; 4]));
    let planes = [(luma.as_slice(), 8), (cb.as_slice(), 4), (cr.as_slice(), 4)];
    let deep = |headroom| DeepPlanes { size: (4, 4), planes, hlg: false, full: false, headroom };
    let mut slot = None;
    let lit = r.convert_deep(&mut slot, &deep(4.0)).unwrap().clone();
    assert_eq!(lit.format(), wgpu::TextureFormat::Rgba16Float);
    let top = r.read_light(&lit).unwrap()[5][..3].iter().copied().fold(0.0, f32::max);
    assert!(top > 1.5 && top <= 4.001, "{top}");
    let plain = r.convert_deep(&mut slot, &deep(1.0)).unwrap();
    assert_eq!(
        plain.format(),
        wgpu::TextureFormat::Rgba8UnormSrgb,
        "an SDR screen gets standard colors again"
    );
    assert!(
        r.convert_deep(&mut None, &deep(f32::NAN)).unwrap().format() == wgpu::TextureFormat::Rgba8UnormSrgb
    );
}
