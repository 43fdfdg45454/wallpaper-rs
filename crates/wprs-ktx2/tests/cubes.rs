use image::{Rgba, Rgba32FImage, RgbaImage};
use wprs_ktx2::{
    Codec, Options, Shape, Target, durations, encode_frames, encode_light, faces, gpu_bytes, info, paced,
    plain, plain_faces, rgba,
};

fn options() -> Options {
    Options { codec: Codec::Uastc, quality: 70, srgb: true, mips: false, repeat: false }
}

fn quarter(x: u32) -> [u8; 3] {
    [[220, 30, 30], [30, 220, 30], [30, 30, 220], [220, 220, 30]][(x / 16) as usize]
}

#[test]
fn a_panorama_becomes_six_faces_that_show_it_again() {
    let panorama = RgbaImage::from_fn(64, 32, |x, y| {
        let [r, g, b] = if y < 6 {
            [250, 250, 250]
        } else if y > 25 {
            [10, 10, 10]
        } else {
            quarter(x)
        };
        Rgba([r, g, b, 255])
    });
    let six = plain_faces(&panorama).unwrap();
    assert_eq!(six.len(), 6);
    assert!(six.iter().all(|f| f.dimensions() == (16, 16)));
    let bytes = encode_frames(&six, &options(), Shape::Cube).unwrap();
    let back = rgba(&bytes, 4096).unwrap();
    assert_eq!(back.dimensions(), (64, 32));
    for (x, y) in [(8, 16), (24, 16), (40, 16), (56, 16), (32, 1), (32, 30)] {
        let (a, b) = (panorama.get_pixel(x, y).0, back.get_pixel(x, y).0);
        assert!(a.iter().zip(b).all(|(p, q)| p.abs_diff(q) <= 40), "at {x},{y}: {a:?} came back as {b:?}");
    }
    assert!(plain_faces(&RgbaImage::new(2, 1)).is_err(), "a panorama too small for a face was cut");
}

#[test]
fn an_hdr_panorama_keeps_its_light_in_a_cube_map() {
    let panorama =
        Rgba32FImage::from_fn(32, 16, |x, _| Rgba([if x < 16 { 6.0 } else { 0.2 }, 0.5, 0.25, 1.0]));
    let layers = faces(&panorama).unwrap();
    let bytes = encode_light(&layers, &options()).unwrap();
    let found = info(&bytes).unwrap();
    assert_eq!((found.cube, found.hdr, found.picture(0)), (true, true, (32, 16)));
    assert!(plain(&found, Target::Bc7), "a cube is drawn as a panorama, never compressed");
    assert_eq!(gpu_bytes(&found, Target::Bc7, true), 32 * 16 * 8, "an HDR cube takes half floats");
    let flat = encode_light(&layers[..1], &options()).unwrap();
    assert!(!info(&flat).unwrap().cube);
    assert!(encode_light(&layers[..2], &options()).is_err(), "two faces made a texture");
}

#[test]
fn arrays_keep_each_frame_delay_and_report_what_the_card_holds() {
    let frames: Vec<RgbaImage> =
        (0..3).map(|i| RgbaImage::from_pixel(6, 6, Rgba([i * 80, 0, 0, 255]))).collect();
    let bytes = encode_frames(&frames, &options(), Shape::Frames).unwrap();
    let mixed = paced(&bytes, &[0.05, 0.5, 2.0]);
    assert_eq!(durations(&mixed).unwrap(), vec![0.05, 0.5, 2.0]);
    let even = paced(&bytes, &[0.25; 3]);
    assert_eq!(durations(&even).unwrap(), vec![0.25; 3]);
    let keys = wprs_ktx2::kv::pairs(&even).iter().map(|(k, _)| k.to_vec()).collect::<Vec<_>>();
    assert!(!keys.contains(&wprs_ktx2::kv::PACE.to_vec()), "even frames kept a list of delays");
    assert_eq!(rgba(&mixed, 4096).unwrap(), rgba(&bytes, 4096).unwrap());
    let found = info(&bytes).unwrap();
    assert!(plain(&found, Target::Bc7) && !plain(&found, Target::Rgba));
    assert_eq!(gpu_bytes(&found, Target::Bc7, false), gpu_bytes(&found, Target::Rgba, false));
    let even_sides: Vec<RgbaImage> = (0..3).map(|_| RgbaImage::new(8, 8)).collect();
    let packed = info(&encode_frames(&even_sides, &options(), Shape::Frames).unwrap()).unwrap();
    assert!(!plain(&packed, Target::Bc7), "frames of whole blocks stay compressed");
}
