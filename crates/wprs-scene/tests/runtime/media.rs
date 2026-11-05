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

#[test]
fn playing_shows_and_hides_layers_and_drives_numbers() {
    let layers = json!([
        { "id": "on", "type": "solid", "color": "#ffffff", "visible": { "media": "playing" } },
        { "id": "lvl", "type": "solid", "color": "#ffffff", "opacity": { "media": "playing", "scale": 0.5 } }
    ]);
    let mut rt = runtime(&scene(layers));
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(frame.draws.is_empty(), "nothing plays, so one layer is hidden and the other clear");
    rt.set_media(playing());
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(frame.draws.len(), 2);
    assert!(close(frame.draws[1].opacity, 0.5) && frame.wake == Wake::Idle);
    rt.set_media(MediaState { playing: false, ..playing() });
    assert!(rt.frame(VIEW, Quality::Q3).draws.is_empty(), "a pause hides them again");
}

#[test]
fn the_position_runs_on_from_the_last_reading_only_while_playing() {
    let layers = json!([
        { "id": "bar", "type": "solid", "color": "#ffffff", "opacity": { "media": "progress", "scale": 0.5, "offset": 0.5 } },
        { "id": "at", "type": "text", "text": { "media": "position" } },
        { "id": "len", "type": "text", "text": { "media": "duration" } },
        { "id": "done", "type": "text", "text": { "media": "progress" } }
    ]);
    let mut rt = runtime(&scene(layers));
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(close(frame.draws[0].opacity, 0.5) && text(&frame, 1) == "0:00" && frame.wake == Wake::Idle);
    rt.set_media(playing());
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(close(frame.draws[0].opacity, 0.525), "10 s of 200 s");
    assert_eq!(
        (text(&frame, 1), text(&frame, 2), text(&frame, 3)),
        ("0:10".into(), "3:20".into(), "5%".into())
    );
    assert_eq!(frame.wake, Wake::Frame, "a bar moves while the track plays");
    rt.advance(5.0);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(close(frame.draws[0].opacity, 0.5375) && text(&frame, 1) == "0:15");
    rt.set_media(MediaState { rate: 2.0, ..playing() });
    rt.advance(5.0);
    assert_eq!(text(&rt.frame(VIEW, Quality::Q3), 1), "0:20", "twice as fast from the new reading");
    rt.set_media(MediaState { playing: false, position: 3725.0, duration: 4000.0, ..playing() });
    rt.advance(30.0);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert_eq!(
        (text(&frame, 1), frame.wake),
        ("1:02:05".into(), Wake::Idle),
        "a pause stops the clock and wakes nothing"
    );
    rt.set_media(MediaState { position: 199.5, ..playing() });
    rt.advance(10.0);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(
        close(frame.draws[0].opacity, 1.0) && frame.wake == Wake::Idle,
        "the end of the track stays still"
    );
}

#[test]
fn a_text_of_the_position_wakes_once_a_second() {
    let mut rt = runtime(&scene(json!([{ "id": "at", "type": "text", "text": { "media": "position" } }])));
    rt.set_media(MediaState { position: 10.25, ..playing() });
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(matches!(frame.wake, Wake::At(t) if close(t as f32, 0.75)), "{:?}", frame.wake);
    rt.set_media(MediaState { position: 10.25, rate: 0.5, ..playing() });
    assert!(matches!(rt.frame(VIEW, Quality::Q3).wake, Wake::At(t) if close(t as f32, 1.5)));
    rt.set_media(MediaState { playing: false, ..playing() });
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
}
