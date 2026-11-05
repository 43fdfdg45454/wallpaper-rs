use super::*;

fn flat(color: [u8; 4]) -> Arc<Pixels> {
    Pixels::new(RgbaImage::from_pixel(16, 16, Rgba(color)))
}

fn covers(r: &Renderer, placeholder: Arc<Pixels>) -> Shot {
    let assets = json!({
        "cover": { "path": "cover.png", "media": "current" },
        "previous": { "path": "cover.png", "media": "previous" }
    });
    let layers = json!([
        { "id": "now", "type": "image", "image": "cover", "size": [50, 50], "transform": { "position": [25, 25] } },
        { "id": "then", "type": "image", "image": "previous", "size": [50, 50], "transform": { "position": [75, 25] } },
        { "id": "fade", "type": "image", "image": "previous", "size": [50, 50], "opacity": 0.5,
          "transform": { "position": [25, 25] } }
    ]);
    let text = json!({ "canvas": canvas("#000000"), "assets": assets, "layers": layers });
    Shot::with(r, text, &[("cover", placeholder.clone()), ("previous", placeholder)])
}

fn show(r: &Renderer, shot: &mut Shot, cover: &Arc<Pixels>, previous: &Arc<Pixels>) -> bool {
    let images = [("cover".to_string(), cover.clone()), ("previous".to_string(), previous.clone())];
    let assets = SceneAssets { images: images.into(), ..Default::default() };
    let scene = shot.runtime.scene.clone();
    r.refresh_scene(&mut shot.view, (&scene, shot.runtime.props()), &assets).unwrap()
}

#[test]
fn covers_replace_their_placeholder_and_the_previous_one_fades_over_the_new_one() {
    let Some(r) = renderer() else { return };
    let grey = flat([128, 128, 128, 255]);
    let mut shot = covers(r, grey.clone());
    check(&shot.pixels(r), &[((25, 25), [128, 128, 128]), ((75, 25), [128, 128, 128])]);
    assert!(shot.runtime.uses().media, "cover assets ask for the media input");
    let (red, blue) = (flat([255, 0, 0, 255]), flat([0, 0, 255, 255]));
    assert!(show(r, &mut shot, &red, &grey), "a new cover is taken");
    assert!(!show(r, &mut shot, &red, &grey), "the same cover changes nothing");
    check(&shot.pixels(r), &[((75, 25), [128, 128, 128])]);
    let mixed = at(&shot.pixels(r), 25, 25);
    assert!(
        mixed[0] > 150 && mixed[1] > 30 && mixed[1] < 110,
        "the placeholder fades over the cover: {mixed:?}"
    );
    assert!(show(r, &mut shot, &blue, &red));
    check(&shot.pixels(r), &[((75, 25), [255, 0, 0])]);
    let mixed = at(&shot.pixels(r), 25, 25);
    assert!(
        mixed[0] > 90 && mixed[2] > 90 && mixed[1] < 20,
        "the last cover fades over the new one: {mixed:?}"
    );
    assert!(show(r, &mut shot, &grey, &grey), "without a track both go back to the placeholder");
    check(&shot.pixels(r), &[((25, 25), [128, 128, 128]), ((75, 25), [128, 128, 128])]);
}
