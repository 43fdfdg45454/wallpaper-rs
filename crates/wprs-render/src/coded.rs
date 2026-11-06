use crate::deep::{GAMUT, KNEE, PEAK_NITS, REFERENCE_NITS};
use crate::{Renderer, yuv};

pub const BT2020: (f32, f32) = (0.2627, 0.0593);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Coding {
    #[default]
    Sdr,
    Pq,
    Hlg,
}

pub(crate) fn lighting(coding: Coding, headroom: f32) -> f32 {
    let fine = headroom.is_finite() && coding != Coding::Sdr;
    if fine { headroom.clamp(1.0, crate::hdr::MAX_HEADROOM) } else { 1.0 }
}

pub(crate) fn relight(queue: &wgpu::Queue, buffer: &wgpu::Buffer, at: u64, light: f32) -> f32 {
    let row: Vec<u8> = [light, 0.0, 0.0, 0.0].iter().flat_map(|v| v.to_le_bytes()).collect();
    queue.write_buffer(buffer, at, &row);
    light
}

pub(crate) fn uniform(renderer: &Renderer, full: bool, coding: Coding, headroom: f32) -> wgpu::Buffer {
    let tone = [if coding == Coding::Hlg { 2.0 } else { 1.0 }, REFERENCE_NITS, PEAK_NITS, KNEE];
    let light = [headroom, 0.0, 0.0, 0.0];
    let values = yuv::rows_of(BT2020, full).into_iter().chain([[1.0, 0.0, 0.0, 1.0], tone]).chain(GAMUT);
    let values = values.chain([light]);
    let contents: Vec<u8> = values.flatten().flat_map(f32::to_le_bytes).collect();
    let usage = wgpu::BufferUsages::UNIFORM;
    crate::filled::filled((&renderer.device, &renderer.queue), "wprs-render coded rows", &contents, usage)
}

pub(crate) fn source() -> &'static str {
    concat!(include_str!("tone.wgsl"), include_str!("coded.wgsl"))
}

#[cfg(test)]
mod tests {
    use super::BT2020;
    use crate::yuv::{rows, rows_of};

    fn near(bt709: bool, full: bool, [y, u, v]: [u8; 3], want: [i32; 3]) -> bool {
        let yuv = [y, u, v].map(|c| f32::from(c) / 255.0);
        let rgb = rows(bt709, full).map(|r| {
            ((r[0] * yuv[0] + r[1] * yuv[1] + r[2] * yuv[2] + r[3]).clamp(0.0, 1.0) * 255.0).round() as i32
        });
        rgb.iter().zip(want).all(|(have, want)| (have - want).abs() <= 2)
    }

    #[test]
    fn studio_and_full_swing_colours_come_back() {
        assert!(near(false, false, [81, 90, 240], [255, 0, 0]));
        assert!(near(true, false, [63, 102, 240], [255, 0, 0]));
        assert!(near(true, false, [173, 42, 26], [0, 255, 0]));
        assert!(near(true, false, [235, 128, 128], [255, 255, 255]));
        assert!(near(false, true, [0, 128, 128], [0, 0, 0]));
        assert!(near(true, true, [128, 128, 128], [128, 128, 128]));
        assert!(!near(false, false, [63, 102, 240], [255, 0, 0]));
    }

    #[test]
    fn wide_gamut_rows_keep_grey_grey_and_white_white() {
        let at = |ycc: [f32; 3]| {
            rows_of(BT2020, false).map(|r| r[0] * ycc[0] + r[1] * ycc[1] + r[2] * ycc[2] + r[3])
        };
        let white = at([235.0 / 255.0, 0.5 + 0.5 / 255.0, 0.5 + 0.5 / 255.0]);
        assert!(white.iter().all(|c| (c - 1.0).abs() < 0.01), "{white:?}");
        let red =
            at([0.2627 * 219.0 / 255.0 + 16.0 / 255.0, (128.0 - 0.1396 * 224.0) / 255.0, 240.0 / 255.0]);
        assert!(red[0] > 0.95 && red[1].abs() < 0.03 && red[2].abs() < 0.03, "{red:?}");
    }
}
