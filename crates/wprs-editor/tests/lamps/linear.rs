use image::{DynamicImage, Rgb32FImage};
use serde_json::json;

fn bright(dir: &std::path::Path, name: &str) {
    let picture = Rgb32FImage::from_pixel(2, 2, image::Rgb([0.5, 0.5, 4.0]));
    DynamicImage::ImageRgb32F(picture).save(dir.join(name)).unwrap();
}

#[test]
fn float_maps_marked_linear_are_read_as_data_and_color_pictures_through_the_srgb_curve() {
    let dir = tempfile::tempdir().unwrap();
    bright(dir.path(), "map.hdr");
    let doc =
        |linear: bool| json!({ "scene": { "assets": { "m": { "path": "map.hdr", "linear": linear } } } });
    let read = |linear| wprs_editor::pictures::asset(dir.path(), &doc(linear), "m").unwrap().into_rgba8();
    assert_eq!(read(true).get_pixel(0, 0).0, [128, 128, 255, 255], "a linear map went through the curve");
    let shown = read(false).get_pixel(0, 0).0;
    assert!(
        (186..=189).contains(&shown[0]) && shown[2] == 255,
        "a color picture skipped the curve: {shown:?}"
    );
    assert!(wprs_editor::pictures::asset(dir.path(), &doc(true), "none").is_err());
}
