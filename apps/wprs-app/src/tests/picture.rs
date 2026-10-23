use crate::picture::{Picture, trouble, write};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use wprs_core::edit::Edit;

#[test]
fn the_editor_draws_its_picture_on_the_gpu() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("grey.png");
    image::RgbaImage::from_pixel(64, 32, image::Rgba([100, 100, 100, 255])).save(&file).unwrap();
    let picture = match Picture::open(&file) {
        Ok(picture) => picture,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let mut edit = Edit::default();
    let plain = picture.draw(&edit, false).unwrap();
    assert_eq!(plain.dimensions(), (64, 32));
    edit.set("ev", "2").unwrap();
    edit.act(serde_json::from_value(json!({"crop": {"rect": "0,0,0.5,1", "rotate": 90}})).unwrap()).unwrap();
    let (cropped, whole) = (picture.draw(&edit, false).unwrap(), picture.draw(&edit, true).unwrap());
    assert_eq!((cropped.dimensions(), whole.dimensions()), ((16, 64), (32, 64)));
    assert!(cropped.get_pixel(8, 32)[0] > plain.get_pixel(8, 16)[0] + 40);
    let path = write(&cropped).unwrap();
    let data = std::fs::read(&path).unwrap();
    assert!(data.starts_with(b"P6\n16 64\n255\n") && data.len() == 13 + 16 * 64 * 3);
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    std::fs::remove_file(path).unwrap();
    assert!(!picture.lost());
}

#[test]
fn the_editor_names_why_a_picture_cannot_be_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("broken.png");
    std::fs::write(&file, b"not a picture").unwrap();
    assert_eq!(trouble(&Picture::open(&file).err().unwrap()), "decode");
    assert_eq!(trouble(&wprs_render::RenderError::NoAdapter("none".into()).into()), "gpu");
    assert_eq!(trouble(&wprs_render::RenderError::DeviceLost("reset".into()).into()), "draw");
}
