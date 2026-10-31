use image::{Luma, Rgb, RgbImage};
use wprs_core::edit::Depth;
use wprs_depth::{Map, adjust, input_size, normalize, shrink, tensor};

#[test]
fn images_become_imagenet_tensors_in_patches_of_14() {
    assert_eq!(input_size((4000, 3000), 518), (686, 518));
    assert_eq!(input_size((3000, 4000), 518), (518, 686));
    assert_eq!(input_size((8000, 1000), 518), (1036, 126));
    let (data, (w, h)) = tensor(&RgbImage::from_pixel(40, 20, Rgb([255, 0, 128])), 28);
    assert_eq!((w, h, data.len()), (56, 28, 3 * 56 * 28));
    let plane = (w * h) as usize;
    let expect = [(1.0 - 0.485) / 0.229, (0.0 - 0.456) / 0.224, (128.0 / 255.0 - 0.406) / 0.225];
    for (c, want) in expect.into_iter().enumerate() {
        assert!((data[c * plane + 17] - want).abs() < 1e-3, "channel {c}: {}", data[c * plane + 17]);
    }
}

#[test]
fn outputs_are_stretched_between_percentiles_at_the_image_shape() {
    let mut raw: Vec<f32> = (0..28 * 14).map(|i| (i % 28) as f32).collect();
    raw[5] = 1e6;
    raw[6] = -1e6;
    let map = normalize(&raw, (28, 14), (64, 30), 14);
    assert_eq!(map.dimensions(), (28, 13));
    let row: Vec<u16> = (0..28).map(|x| map.get_pixel(x, 9)[0]).collect();
    assert!(row.windows(2).all(|w| w[0] <= w[1]) && row[0] == 0 && row[27] == u16::MAX, "{row:?}");
}

#[test]
fn adjustments_remap_dilate_and_smooth() {
    let ramp = Map::from_fn(11, 11, |x, _| Luma([(x * 6553) as u16]));
    let flipped = adjust(&ramp, &Depth { near: 0.0, far: 1.0, smooth: 0, dilate: 0, ..Depth::default() });
    assert_eq!((flipped.get_pixel(0, 0)[0], flipped.get_pixel(10, 0)[0]), (u16::MAX, 5));
    let curved = adjust(&ramp, &Depth { gamma: 2.0, smooth: 0, dilate: 0, ..Depth::default() });
    assert!(curved.get_pixel(5, 0)[0].abs_diff(16384) < 64);
    let dot = Map::from_fn(11, 11, |x, y| Luma([if (x, y) == (5, 5) { u16::MAX } else { 0 }]));
    let grown = adjust(&dot, &Depth { smooth: 0, dilate: 2, ..Depth::default() });
    let lit = grown.pixels().filter(|p| p[0] == u16::MAX).count();
    assert_eq!((lit, grown.get_pixel(3, 7)[0], grown.get_pixel(2, 5)[0]), (25, u16::MAX, 0));
    let soft = adjust(&dot, &Depth { smooth: 1, dilate: 0, ..Depth::default() });
    assert_eq!((soft.get_pixel(5, 5)[0], soft.get_pixel(4, 4)[0], soft.get_pixel(3, 5)[0]), (7281, 7281, 0));
    assert_eq!(shrink(Map::new(4000, 1000), 1024).dimensions(), (1024, 256));
}

#[test]
fn the_manifest_pins_permissive_files_on_github() {
    use wprs_depth::models::{assets, find};
    use wprs_depth::{Asset, MODEL, RUNTIME};
    for asset in assets() {
        assert!(!asset.urls.is_empty() && asset.urls.iter().all(|u| u.starts_with("https://github.com/")));
        assert!(asset.sha256.len() == 64 && asset.file_sha256.len() == 64 && asset.bytes > 1 << 20);
        assert!(
            ["Apache-2.0", "MIT"].contains(&asset.license)
                && asset.license_url.starts_with("https://github.com/")
        );
        assert_eq!((find(asset.id), asset.hosts().as_str()), (Some(asset), "github.com"));
    }
    assert_eq!((MODEL.bytes, MODEL.urls.len()), (99_092_268, 1));
    assert!(MODEL.license_text.contains("Apache License\n                           Version 2.0"));
    assert!(RUNTIME.is_none_or(|r| r.license_text.starts_with("MIT License") && r.license == "MIT"));
    assert!(
        MODEL.urls[0].contains("/v2.0.0/")
            && MODEL.archive() == MODEL.file
            && MODEL.notice().contains(MODEL.sha256)
    );
    assert!(RUNTIME.is_none_or(|r| r.member.ends_with(r.file) && r.archive().ends_with(".tgz")));
    assert!(RUNTIME.is_none_or(|r| r.notices.ends_with("/ThirdPartyNotices.txt")));
    let urls = &["https://github.com/a", "https://mirror.example/b", "https://github.com/c"];
    assert_eq!(Asset { urls, ..MODEL }.hosts(), "github.com, mirror.example");
    assert_eq!(find("depth-anything-v2-large"), None);
}

#[test]
fn heif_photos_reach_the_depth_maps_through_the_platform_decoder() {
    let heic = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/blue-64x32.heic");
    assert!(wprs_depth::shape(&heic).is_err());
    let decode = |_: &std::path::Path, edge: u32| {
        Some(image::RgbaImage::from_pixel(edge.min(64), edge.min(32), image::Rgba([9; 4])))
    };
    assert!(wprs_render::set_platform_decoder(wprs_render::Platform { decode, size: |_| Some((64, 32)) }));
    assert_eq!(wprs_depth::shape(&heic).unwrap(), (64, 32));
    #[cfg(feature = "infer")]
    assert_eq!(wprs_depth::decode(&heic).unwrap().dimensions(), (64, 32));
}
