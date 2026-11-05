use super::*;
use wprs_ktx2::{Codec, Options, Target, encode};

fn gradient() -> RgbaImage {
    RgbaImage::from_fn(64, 32, |x, y| Rgba([(x * 4) as u8, (y * 8) as u8, 200 - (x * 2) as u8, 255]))
}

fn packed(image: &RgbaImage, codec: Codec, mips: bool) -> Arc<Pixels> {
    let options = Options { codec, quality: 90, srgb: true, mips, repeat: false };
    Pixels::ktx2(encode(image, &options).unwrap()).unwrap()
}

fn shot(r: &Renderer, asset: Value, size: [u32; 2], pixels: Arc<Pixels>) -> Vec<u8> {
    let layer = json!({ "id": "p", "type": "image", "image": "p", "size": size });
    let text = json!({ "canvas": canvas("#000000"), "assets": { "p": asset }, "layers": [layer] });
    Shot::with(r, text, &[("p", pixels)]).pixels(r)
}

fn apart(a: &[u8], b: &[u8]) -> f64 {
    a.iter().zip(b).map(|(x, y)| f64::from(x.abs_diff(*y))).sum::<f64>() / a.len() as f64
}

#[test]
fn ktx2_pictures_look_like_their_source_on_every_target_the_card_offers() {
    let Some(r) = renderer() else { return };
    let source = gradient();
    let asset = json!({ "path": "p.ktx2" });
    let plain = shot(r, json!({ "path": "p.png" }), [100, 50], Pixels::new(source.clone()));
    eprintln!("targets offered: {:?}", r.ktx2_targets());
    assert_eq!(r.ktx2_targets().last(), Some(&Target::Rgba));
    for target in r.ktx2_targets() {
        r.force_ktx2(Some(target));
        assert_eq!(r.ktx2_target(), target);
        for codec in [Codec::Uastc, Codec::Etc1s] {
            let seen = shot(r, asset.clone(), [100, 50], packed(&source, codec, true));
            let off = apart(&plain, &seen);
            assert!(off < 6.0, "{target:?} {codec:?}: {off}");
        }
    }
    r.force_ktx2(None);
}

#[test]
fn ktx2_pictures_with_odd_sides_stay_compressed_and_exact() {
    let Some(r) = renderer() else { return };
    let source = RgbaImage::from_fn(30, 18, |x, y| match (x >= 27, y >= 15) {
        (true, _) => Rgba([255, 0, 0, 255]),
        (_, true) => Rgba([0, 0, 255, 255]),
        _ => Rgba([0, 200, 0, 255]),
    });
    let plain = shot(r, json!({ "path": "p.png" }), [90, 54], Pixels::new(source.clone()));
    for target in r.ktx2_targets() {
        r.force_ktx2(Some(target));
        let seen = shot(r, json!({ "path": "p.ktx2" }), [90, 54], packed(&source, Codec::Uastc, false));
        let off = apart(&plain, &seen);
        assert!(off < 12.0, "{target:?}: {off}");
        for ((x, y), channel) in [((83, 25), 1), ((91, 25), 0), ((40, 41), 1), ((40, 49), 2)] {
            let c = at(&seen, x, y);
            let top = (0..3).max_by_key(|i| c[*i]).unwrap();
            assert!(top == channel && c[channel] > 150, "{target:?} at {x},{y}: {c:?}");
        }
    }
    r.force_ktx2(None);
}

#[test]
fn ktx2_mipmaps_come_from_the_file_and_can_be_left_out() {
    let Some(r) = renderer() else { return };
    let checker =
        RgbaImage::from_fn(64, 64, |x, y| Rgba(if (x + y) % 2 == 0 { [0, 0, 0, 255] } else { [255; 4] }));
    let grey = |asset: Value, pixels: Arc<Pixels>| {
        let shot = shot(r, asset, [20, 20], pixels);
        (42..58)
            .flat_map(|x| (17..33).map(move |y| (x, y)))
            .map(|(x, y)| at(&shot, x, y)[0])
            .collect::<Vec<_>>()
    };
    for target in r.ktx2_targets() {
        r.force_ktx2(Some(target));
        let mipped = grey(json!({ "path": "c.ktx2" }), packed(&checker, Codec::Uastc, true));
        assert!(mipped.iter().all(|v| (120..235).contains(v)), "{target:?}: {mipped:?}");
        let sharp = grey(json!({ "path": "c.ktx2", "mipmaps": false }), packed(&checker, Codec::Uastc, true));
        assert!(sharp.iter().any(|v| !(120..235).contains(v)), "{target:?}: {sharp:?}");
        let blitted =
            grey(json!({ "path": "c.ktx2", "mipmaps": true }), packed(&checker, Codec::Uastc, false));
        assert!(blitted.iter().all(|v| (120..235).contains(v)), "{target:?}: {blitted:?}");
    }
    r.force_ktx2(None);
}

#[test]
fn ktx2_pictures_are_read_like_other_pictures() {
    let bytes = encode(
        &gradient(),
        &Options { codec: Codec::Uastc, quality: 90, srgb: true, mips: true, repeat: false },
    )
    .unwrap();
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes.clone()));
    let decoded = wprs_render::decode_limited(reader, wprs_render::picture_limits()).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (64, 32));
    assert!(apart(decoded.as_raw(), gradient().as_raw()) < 6.0);
    let mut tight = wprs_render::picture_limits();
    tight.max_image_width = Some(32);
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes.clone()));
    assert!(wprs_render::decode_limited(reader, tight).is_err());
    let mut assets = SceneAssets::default();
    let asset: wprs_scene::Asset = serde_json::from_value(json!({ "path": "p.ktx2" })).unwrap();
    let mut pictures = wprs_render::Pictures::default();
    let total = pictures
        .add::<image::ImageError>((&mut assets, "p", &asset), || Ok(bytes), |_| unreachable!())
        .unwrap();
    assert_eq!(total, 64 * 32 * 4);
    assert!(assets.images["p"].packed().is_some() && !assets.images["p"].resident());
    assert_eq!(assets.images["p"].image().unwrap().dimensions(), (64, 32));
    let other: wprs_scene::Asset = serde_json::from_value(json!({ "path": "q.ktx2" })).unwrap();
    let read = || Ok(wprs_ktx2::MAGIC.to_vec());
    let broken = pictures.add::<image::ImageError>((&mut assets, "q", &other), read, |_| unreachable!());
    assert!(broken.is_err());
}

#[test]
fn the_card_draws_each_ktx2_class_it_reads_like_the_processor_decodes_it() {
    let Some(r) = renderer() else { return };
    let source = gradient();
    let options = Options { codec: Codec::Uastc, quality: 90, srgb: true, mips: false, repeat: false };
    let bytes = encode(&source, &options).unwrap();
    assert!(r.look_ktx2(&bytes, Target::Rgba).is_none(), "plain pixels need no drawing");
    for target in r.ktx2_targets().into_iter().filter(|t| t.compressed()) {
        let drawn = r.look_ktx2(&bytes, target).unwrap();
        let decoded = wprs_ktx2::look(&bytes, target).unwrap();
        assert_eq!(drawn.dimensions(), (64, 32));
        let off = apart(drawn.as_raw(), decoded.as_raw());
        assert!(off < 3.0, "{target:?}: {off}");
    }
}
