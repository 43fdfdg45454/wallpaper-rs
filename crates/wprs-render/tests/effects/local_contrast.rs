use super::*;
use wprs_scene::Quality;

fn soft() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, _| {
        let v = if x < W / 2 { 100 } else { 140 };
        Rgba([v, v / 2, 60, 255])
    })
}

#[test]
fn local_contrast_deepens_edges_and_keeps_flat_parts() {
    let fx = |params: Value| Fx::new("local_contrast", params).input(soft());
    let Some(luma) = fx(json!({ "radius": 6, "amount": 4 })).shot() else { return };
    let [dark, ..] = luma.at(30, 16);
    let [light, ..] = luma.at(33, 16);
    assert!(dark < 92 && light > 148, "{dark} {light}");
    luma.check(&[((2, 16), [100, 50, 60]), ((61, 16), [140, 70, 60])]);
    let Some(colored) = fx(json!({ "radius": 6, "amount": 4, "luma_only": false })).shot() else { return };
    assert!(colored.diff(&luma) > 0.2);
    let Some(low) = fx(json!({ "radius": 6, "amount": 2 })).quality(Quality::Q0).shot() else { return };
    assert_eq!(low.diff(&Shot(soft())), 0.0);
    Shot::sheet(&[luma, colored]).looks_like("local_contrast");
}
