use super::{frame, split};
use serde_json::json;
use std::path::Path;

#[test]
fn the_simulated_player_leaves_the_document_and_becomes_a_frame() {
    let doc =
        json!({"scene": {"layers": []}, "preview": {"quality": "q1", "media": {"title": "Night Drive"}}});
    let (rest, media) = split(&doc.to_string());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rest).unwrap()["preview"],
        json!({"quality": "q1"})
    );
    assert_eq!(media, Some(json!({"title": "Night Drive"})));
    let plain = r#"{"scene": {}}"#;
    assert_eq!(split(plain), (plain.to_owned(), None));
    let made = json!({"title": "Night Drive", "artist": ["First", "Second"], "genres": "Ambient",
        "playing": true, "position": 30, "duration": 20});
    let f = frame(Path::new("."), made).unwrap();
    let s = &f.state;
    assert!(s.present() && s.playing && s.enabled);
    assert_eq!(
        (s.title.as_str(), s.artist.as_str(), s.genres.as_str()),
        ("Night Drive", "First, Second", "Ambient")
    );
    assert_eq!((s.position, s.duration, s.rate, s.kind.as_str()), (20.0, 20.0, 1.0, "music"));
}

#[test]
fn the_simulated_player_refuses_bad_values_and_caps() {
    let refused = |v: serde_json::Value| frame(Path::new("."), v).map(drop).unwrap_err().to_string();
    assert!(refused(json!({"title": "x".repeat(513)})).contains("512"));
    assert!(refused(json!({"title": "a\u{7}b"})).contains("control"));
    assert!(refused(json!({"position": -1})).contains("media.position"));
    assert!(refused(json!({"duration": 1e8})).contains("media.duration"));
    assert!(refused(json!({"rate": 500})).contains("media.rate"));
    assert!(refused(json!({"lyrics": "la"})).contains("unknown field"));
    assert!(refused(json!("Night Drive")).contains("simulated media player"));
    assert!(frame(Path::new("."), json!({"artist": vec!["x"; 4]})).is_ok());
}

#[test]
fn a_simulated_cover_is_a_picture_inside_the_scene_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    image::RgbaImage::from_pixel(300, 300, image::Rgba([0, 0, 255, 255]))
        .save(dir.path().join("assets/cover.png"))
        .unwrap();
    let f = frame(dir.path(), json!({"title": "x", "cover": "assets/cover.png"})).unwrap();
    let cover = f.cover.as_ref().unwrap();
    assert_eq!((cover.pixels.size(), f.state.cover), ((256, 256), cover.serial));
    for wrong in ["../cover.png", "/etc/passwd", "assets/missing.png"] {
        let refused = frame(dir.path(), json!({"cover": wrong})).map(drop).unwrap_err().to_string();
        assert!(refused.contains("media.cover"), "{wrong}: {refused}");
    }
    assert!(frame(dir.path(), json!({"cover": ""})).unwrap().cover.is_none());
}
