use super::renderer;
use image::{Rgba, Rgba32FImage, RgbaImage};
use wprs_ktx2::{Codec, Options, encode, encode_light};
use wprs_render::{bright_picture, light_texels};

fn options() -> Options {
    Options { codec: Codec::Uastc, quality: 90, srgb: true, mips: false, repeat: false }
}

fn texels(values: &[[f32; 4]]) -> Vec<u8> {
    values.iter().flatten().flat_map(|v| half::f16::from_f32(*v).to_le_bytes()).collect()
}

fn read(data: &[u8], texel: usize) -> [f32; 4] {
    let at =
        |i: usize| half::f16::from_le_bytes([data[texel * 8 + 2 * i], data[texel * 8 + 2 * i + 1]]).to_f32();
    [at(0), at(1), at(2), at(3)]
}

#[test]
fn the_light_of_a_picture_bends_softly_toward_the_peak_of_the_screen() {
    let mut data = texels(&[[0.25, 0.4, 0.1, 1.0], [8.0, 4.0, 0.0, 2.0], [f32::NAN, -1.0, 3.0, 1.0]]);
    light_texels(&mut data, 4.0);
    assert_eq!(read(&data, 0), read(&texels(&[[0.25, 0.4, 0.1, 1.0]]), 0), "light under the knee moved");
    let [r, g, b, a] = read(&data, 1);
    assert!((r - 4.0).abs() < 0.01 && (g - 2.0).abs() < 0.01 && b == 0.0 && a == 1.0, "{r} {g} {b} {a}");
    let [r, g, b, _] = read(&data, 2);
    assert!(r == 0.0 && g == 0.0 && b > 2.0 && b < 3.0, "{r} {g} {b}");
    let mut dim = texels(&[[3.0, 0.2, 0.2, 1.0]]);
    light_texels(&mut dim, 8.0);
    assert_eq!(read(&dim, 0)[0], 3.0, "light the screen can show was bent");
    let mut flat = texels(&[[3.0, 0.2, 0.2, 1.0]]);
    light_texels(&mut flat, f32::NAN);
    assert!((read(&flat, 0)[0] - 1.0).abs() < 1e-3, "a screen without a known peak shows white at most");
}

#[test]
fn hdr_ktx2_pictures_keep_their_light_on_hdr_screens_only() {
    let dir = tempfile::tempdir().unwrap();
    let light = Rgba32FImage::from_fn(16, 8, |x, _| Rgba([if x < 8 { 6.0 } else { 0.5 }, 0.3, 0.2, 1.0]));
    let (bright, plain, png) =
        (dir.path().join("sun.ktx2"), dir.path().join("p.ktx2"), dir.path().join("p.png"));
    std::fs::write(&bright, encode_light(&[light], &options()).unwrap()).unwrap();
    let picture = RgbaImage::from_pixel(16, 8, Rgba([200, 100, 50, 255]));
    std::fs::write(&plain, encode(&picture, &options()).unwrap()).unwrap();
    picture.save(&png).unwrap();
    assert!(bright_picture(&bright));
    assert!(
        !bright_picture(&plain) && !bright_picture(&png) && !bright_picture(&dir.path().join("none.ktx2"))
    );
    let Some(r) = renderer() else { return };
    assert!(r.upload_light(&bright, 1.0).unwrap().is_none(), "a standard screen got the light");
    assert!(r.upload_light(&plain, 4.0).unwrap().is_none() && r.upload_light(&png, 4.0).unwrap().is_none());
    let image = r.upload_light(&bright, 4.0).unwrap().unwrap();
    assert_eq!(image.format(), wgpu::TextureFormat::Rgba16Float);
    let texels = r.read_light(&image).unwrap();
    let (sun, sky) = (texels[2][0], texels[13][0]);
    assert!(sun > 2.5 && sun <= 4.01, "the light above white was {sun}");
    assert!((sky - 0.5).abs() < 0.1, "the light under white moved to {sky}");
}
