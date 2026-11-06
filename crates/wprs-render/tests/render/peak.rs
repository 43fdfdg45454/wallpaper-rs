use super::renderer;
use wprs_render::{Coding, DeepPlanes, GpuImage, Planes};

fn bytes(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn a_new_screen_peak_keeps_the_video_textures_and_only_moves_the_highlights() {
    let Some(r) = renderer() else { return };
    let (luma, cb, cr) = (bytes(&[880; 16]), bytes(&[512; 4]), bytes(&[512; 4]));
    let planes = [(luma.as_slice(), 8), (cb.as_slice(), 4), (cr.as_slice(), 4)];
    let deep = |headroom| DeepPlanes { size: (4, 4), planes, hlg: false, full: false, headroom };
    let mut slot = None;
    let top = |image: &GpuImage| r.read_light(image).unwrap()[5][..3].iter().copied().fold(0.0, f32::max);
    let first = r.convert_deep(&mut slot, &deep(4.0)).unwrap().clone();
    let high = top(&first);
    let again = r.convert_deep(&mut slot, &deep(2.0)).unwrap().clone();
    assert_eq!(again.id(), first.id(), "a new peak built new video textures");
    let low = top(&again);
    assert!(low < high && low <= 2.001, "the highlights did not follow the peak: {high} then {low}");
    let (y, c) = (vec![220u8; 16], vec![128u8; 8]);
    let coded = |headroom| Planes {
        size: (4, 4),
        luma: (&y, 4),
        chroma: (&c, 4),
        bt709: true,
        full: false,
        coding: Coding::Pq,
        headroom,
    };
    let mut yuv = None;
    let one = r.convert_yuv(&mut yuv, &coded(4.0)).unwrap().id();
    assert_eq!(r.convert_yuv(&mut yuv, &coded(3.0)).unwrap().id(), one, "8-bit HDR video rebuilt its slot");
}
