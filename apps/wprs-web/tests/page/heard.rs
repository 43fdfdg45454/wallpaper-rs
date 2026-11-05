use super::helper::{Page, available, start, with};
use base64::Engine;
use serde_json::json;
use wprs_web::{Isolation, ToWeb};

fn png() -> String {
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    base64::engine::general_purpose::STANDARD.encode(out.into_inner())
}

#[test]
fn the_page_hears_the_media_player_with_its_cover_as_a_blob() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let mut page =
        Page::open(start(dir.path(), 9), Isolation { network: true, devices: false, limits: false });
    assert_eq!(page.wait("media", 20), json!(false), "nothing is heard before the daemon says so");
    let colors = json!({ "primary": "#ff0000", "secondary": "#00ff00", "tertiary": "#0000ff", "text": "#ffffff", "contrast": "#000000" });
    let heard = json!({ "enabled": true, "state": "playing", "title": "Night Drive", "artist": "Example Artist",
        "album": "Example Album", "colors": colors, "position": 10.0, "duration": 200.0, "rate": 1.0, "cover": png() });
    page.link.send(&ToWeb::Media(heard), &[]).unwrap();
    page.said.remove("media");
    assert_eq!(page.wait("media", 10), json!(["playing", "Night Drive", "#ff0000", true]));
    assert_eq!(page.wait("cover", 10), json!(["blob:", "image/png", true]));
    page.said.remove("media");
    let paused = json!({ "enabled": true, "state": "paused", "title": "Night Drive", "colors": null, "position": 12.0, "duration": 200.0, "rate": 1.0 });
    page.link.send(&ToWeb::Media(paused), &[]).unwrap();
    assert_eq!(page.wait("media", 10), json!(["paused", "Night Drive", null, true]), "the cover stays");
    page.said.remove("media");
    page.link.send(&ToWeb::Media(json!({ "enabled": false, "cover": "" })), &[]).unwrap();
    assert_eq!(page.wait("media", 10), json!(false));
}

const SLOW: &str = r#"wprs.onMedia((m) => {
  console.log(`probe media ${JSON.stringify(m.enabled ? [m.state, m.title, Boolean(m.cover)] : false)}`);
  if (m.cover) fetch(m.cover).then((r) => r.blob()).then((b) => console.log(`probe cover ${JSON.stringify(b.type)}`));
});
console.log("probe asked true");
const end = Date.now() + 3000;
while (Date.now() < end) {}
"#;

#[test]
fn a_cover_heard_before_the_page_has_loaded_reaches_it_once_it_has() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let html = b"<!doctype html><html><body><script src=\"slow.js\"></script></body></html>";
    let start = with(dir.path(), 9, &[("index.html", html), ("slow.js", SLOW.as_bytes())]);
    let mut page = Page::open(start, Isolation { network: true, devices: false, limits: false });
    assert_eq!(page.wait("asked", 20), json!(true));
    let heard = json!({ "enabled": true, "state": "playing", "title": "Night Drive", "colors": null,
        "position": 1.0, "duration": 200.0, "rate": 1.0, "cover": png() });
    page.link.send(&ToWeb::Media(heard), &[]).unwrap();
    assert_eq!(page.wait("cover", 20), json!("image/png"), "the cover reached the page once it loaded");
    assert_eq!(page.said["media"].last(), Some(&json!(["playing", "Night Drive", true])));
}
