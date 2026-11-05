use super::*;

fn playing() -> MediaState {
    MediaState {
        enabled: true,
        track: 1,
        title: "Night Drive".into(),
        artist: "First, Second".into(),
        album: "Example Album".into(),
        album_artist: "Example Band".into(),
        genres: "Ambient, Drone".into(),
        kind: "music".into(),
        playing: true,
        position: 10.0,
        duration: 200.0,
        rate: 1.0,
        ..MediaState::default()
    }
}

fn text(frame: &Frame, i: usize) -> String {
    match &frame.draws[i].content {
        Content::Text { text, .. } => text.to_string(),
        other => panic!("not a text: {other:?}"),
    }
}

#[test]
fn media_texts_follow_the_track_and_stay_empty_without_one() {
    let names = ["title", "artist", "album", "album_artist", "subtitle", "genres", "kind"];
    let layers: Vec<_> = names
        .iter()
        .map(|n| json!({ "id": format!("t_{n}"), "type": "text", "text": { "media": n } }))
        .collect();
    let mut rt = runtime(&scene(json!(layers)));
    assert!(rt.uses().media);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!((0..7).all(|i| text(&frame, i).is_empty()), "nothing plays, so every text is empty");
    assert!(rt.set_media(playing()) && !rt.set_media(playing()));
    let frame = rt.frame(VIEW, Quality::Q3);
    let shown: Vec<String> = (0..7).map(|i| text(&frame, i)).collect();
    assert_eq!(
        shown,
        ["Night Drive", "First, Second", "Example Album", "Example Band", "", "Ambient, Drone", "music"]
    );
    assert_eq!(frame.wake, Wake::Idle, "texts of a track wake nothing by themselves");
    assert!(rt.set_media(MediaState { enabled: false, ..playing() }));
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(text(&frame, 0).is_empty(), "without the grant the texts are empty");
    let mut limited = runtime(&scene(json!(layers)));
    limited.declare(Uses { media: false, ..Uses::default() });
    assert!(!limited.uses().media && !limited.set_media(playing()));
}

fn solid(frame: &Frame, i: usize) -> Color {
    match &frame.draws[i].content {
        Content::Solid { color, .. } => *color,
        other => panic!("not a solid: {other:?}"),
    }
}

#[test]
fn cover_colours_reach_colours_and_ease_with_smooth() {
    let layers = json!([
        { "id": "p", "type": "solid", "color": { "media": "primary" } },
        { "id": "t", "type": "solid", "color": { "media": "text" } },
        { "id": "c", "type": "solid", "color": { "media": "contrast", "smooth": 1 } }
    ]);
    let mut rt = runtime(&scene(layers));
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(solid(&frame, 0), Color(FALLBACK[0]), "no cover and no placeholder colours give the fallback");
    let colors =
        [[0.9, 0.1, 0.1, 1.0], [0.1, 0.1, 0.9, 1.0], [0.9, 0.9, 0.1, 1.0], [1.0; 4], [0.0, 0.0, 0.0, 1.0]];
    rt.set_media(MediaState { colors: Some(colors), ..playing() });
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!((solid(&frame, 0), solid(&frame, 1)), (Color(colors[0]), Color(colors[3])));
    let eased = solid(&frame, 2).0;
    assert!(
        eased[0] > 0.9 && frame.wake == Wake::Frame,
        "the smoothed colour starts where it was: {eased:?}"
    );
    rt.advance(0.1);
    let half = solid(&rt.frame(VIEW, Quality::Q3), 2).0;
    assert!(half[0] > 0.05 && half[0] < 0.95, "the smoothed colour moves towards black: {half:?}");
    for _ in 0..200 {
        rt.advance(0.1);
        rt.frame(VIEW, Quality::Q3);
    }
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(solid(&frame, 2).0[0] < 0.01 && frame.wake == Wake::Idle);
}
