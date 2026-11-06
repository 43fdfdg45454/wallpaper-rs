use super::*;
use image::{DynamicImage, ImageFormat, Rgb, Rgb32FImage};
use std::io::Cursor;
use wprs_ktx2::{Codec, Options, encode_light};

fn bright() -> Rgb32FImage {
    Rgb32FImage::from_fn(64, 32, |x, y| match x < 32 {
        true => Rgb([0.5, 0.5, 0.5]),
        false => Rgb([0.05 + y as f32 / 40.0, 0.2, 3.0]),
    })
}

fn saved(format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgb32F(bright()).write_to(&mut bytes, format).unwrap();
    bytes.into_inner()
}

fn drawn(bytes: Vec<u8>) -> Arc<Pixels> {
    let reader = image::ImageReader::new(Cursor::new(bytes));
    Pixels::new(wprs_render::decode_asset(reader, wprs_render::picture_limits(), false).unwrap())
}

fn shot(r: &Renderer, path: &str, pixels: Arc<Pixels>) -> Vec<u8> {
    let layer = json!({ "id": "p", "type": "image", "image": "p", "size": [100, 50] });
    let assets = json!({ "p": { "path": path } });
    let text = json!({ "canvas": canvas("#000000"), "assets": assets, "layers": [layer] });
    Shot::with(r, text, &[("p", pixels)]).pixels(r)
}

#[test]
fn hdr_and_exr_pictures_are_drawn_with_the_light_of_their_ktx2_texture() {
    let Some(r) = renderer() else { return };
    let options = Options { codec: Codec::Uastc, quality: 90, srgb: true, mips: false, repeat: false };
    let wide = DynamicImage::ImageRgb32F(bright()).into_rgba32f();
    let packed = Pixels::ktx2(encode_light(&[wide], &options).unwrap()).unwrap();
    let texture = shot(r, "p.ktx2", packed);
    for (path, format) in [("p.hdr", ImageFormat::Hdr), ("p.exr", ImageFormat::OpenExr)] {
        let seen = shot(r, path, drawn(saved(format)));
        let grey = at(&seen, 25, 25);
        assert!(grey.iter().all(|c| c.abs_diff(188) <= 3), "{path}: half the light is {grey:?}");
        let off = seen.iter().zip(&texture).map(|(a, b)| f64::from(a.abs_diff(*b))).sum::<f64>();
        let off = off / seen.len() as f64;
        assert!(off < 4.0, "{path} and its KTX2 texture differ by {off}");
    }
}

#[test]
fn linear_hdr_pictures_keep_their_values_as_data() {
    let reader = image::ImageReader::new(Cursor::new(saved(ImageFormat::OpenExr)));
    let data = wprs_render::decode_asset(reader, wprs_render::picture_limits(), true).unwrap();
    assert_eq!(data.get_pixel(0, 0).0, [128, 128, 128, 255]);
    let picture = drawn(saved(ImageFormat::Hdr)).image().unwrap();
    assert_eq!(picture.get_pixel(0, 0).0[..3], [188, 188, 188]);
    assert_eq!(picture.get_pixel(40, 0).0[2], 255, "light above white is clamped at white");
}
