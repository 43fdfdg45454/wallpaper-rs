use super::*;
use wprs_scene::MediaState;

fn track(title: &str, playing: bool, cover: u64) -> MediaState {
    let colors = (cover != 0).then_some([
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        [1.0; 4],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    MediaState {
        enabled: true,
        track: 1 + u64::from(title != "One"),
        cover,
        title: title.into(),
        artist: "Example Artist".into(),
        kind: "music".into(),
        playing,
        position: 10.0,
        duration: 100.0,
        rate: 1.0,
        colors,
        ..MediaState::default()
    }
}

const LISTENER: &str = r#"
    import { scene, media } from "wprs";
    const log = [];
    const out = scene.layer("out");
    const say = (s) => { log.push(s); out.text = log.join(" "); };
    export function mediaStatus(e) { say(`status:${e.enabled}`); }
    export function mediaPlayback(e) { say(`playback:${e.state}`); }
    export function mediaProperties(e) { say(`props:${e.title}/${e.artist}/${e.kind}`); }
    export function mediaThumbnail(e) { say(`thumb:${e.has_cover}/${e.colors ? e.colors.primary.r : "none"}`); }
    export function mediaTimeline(e) { say(`time:${Math.round(e.position)}/${e.duration}`); }
    export function update() { scene.layer("at").text = `${media.state} ${Math.round(media.position)} ${media.title}`; }
"#;

fn scene(uses: serde_json::Value) -> serde_json::Value {
    let events =
        ["update", "mediaStatus", "mediaPlayback", "mediaProperties", "mediaThumbnail", "mediaTimeline"];
    json!({ "scripts": [{ "path": "main.js", "events": events, "uses": uses }],
        "layers": [{ "id": "out", "type": "text", "text": "", "size": 10 }, { "id": "at", "type": "text", "text": "", "size": 10 }] })
}

#[test]
fn scripts_hear_media_events_and_read_the_last_snapshot() {
    let mut script = staged(scene(json!(["media"])), &[("main.js", LISTENER)]);
    let off = MediaState::default();
    let first = script.update(&Frame { media: Some((&off, 0.0)), ..Frame::at(0.0) }).unwrap();
    assert_eq!(text(&first, "out"), "status:false");
    assert_eq!(text(&first, "at"), "stopped 0 ");
    let one = track("One", true, 0);
    let next = script.update(&Frame { media: Some((&one, 1.0)), ..Frame::at(1.0) }).unwrap();
    assert_eq!(
        text(&next, "out"),
        "status:false status:true playback:playing props:One/Example Artist/music time:10/100"
    );
    let later = script.update(&Frame { media: Some((&one, 1.0)), ..Frame::at(6.0) }).unwrap();
    assert_eq!(text(&later, "at"), "playing 15 One", "the position runs on without a new reading");
    let two = MediaState { playing: false, ..track("Two", false, 4) };
    let paused = script.update(&Frame { media: Some((&two, 7.0)), ..Frame::at(7.0) }).unwrap();
    assert!(
        text(&paused, "out")
            .ends_with("playback:paused props:Two/Example Artist/music thumb:true/1 time:10/100"),
        "{}",
        text(&paused, "out")
    );
    let seeked = MediaState { position: 60.0, ..two.clone() };
    let moved = script.update(&Frame { media: Some((&seeked, 8.0)), ..Frame::at(8.0) }).unwrap();
    assert!(text(&moved, "out").ends_with("time:60/100"));
    let gone = MediaState::default();
    let revoked = script.update(&Frame { media: Some((&gone, 9.0)), ..Frame::at(9.0) }).unwrap();
    let said = text(&revoked, "out");
    assert!(said.ends_with("status:false playback:stopped props:// thumb:false/none time:0/0"), "{said}");
    assert_eq!(text(&revoked, "at"), "stopped 0 ", "without the grant nothing of the track is left");
}

#[test]
fn media_needs_its_declaration_and_carries_no_cover_bytes() {
    let mut script = staged(scene(json!([])), &[("main.js", LISTENER)]);
    let one = track("One", true, 9);
    let out = script.update(&Frame { media: Some((&one, 0.0)), ..Frame::at(0.0) }).unwrap();
    assert!(
        text(&out, "out").starts_with("status:true"),
        "declared events reach the module: {}",
        text(&out, "out")
    );
    let reader = r#"import { scene, media } from "wprs";
        export function update() { scene.layer("at").text = JSON.stringify(media.snapshot()); }"#;
    let mut plain = staged(
        json!({ "scripts": [{ "path": "main.js", "uses": ["layers"] }],
        "layers": [{ "id": "at", "type": "text", "text": "", "size": 10 }] }),
        &[("main.js", reader)],
    );
    let out = plain.update(&Frame { media: Some((&one, 0.0)), ..Frame::at(0.0) }).unwrap();
    assert!(
        text(&out, "at").is_empty() && out.console.iter().any(|l| l.contains("without declaring")),
        "{out:?}"
    );
    let mut script = staged(scene(json!(["media"])), &[("main.js", reader)]);
    let out = script.update(&Frame { media: Some((&one, 0.0)), ..Frame::at(0.0) }).unwrap();
    let shown: serde_json::Value = serde_json::from_str(&text(&out, "at")).unwrap();
    assert_eq!(shown["has_cover"], true);
    assert_eq!(shown["colors"]["primary"], json!({ "r": 1, "g": 0, "b": 0, "a": 1 }));
    assert!(shown.get("cover").is_none() && shown.get("at").is_none(), "{shown}");
}

#[test]
fn the_media_template_of_the_editor_feeds_values() {
    let source = include_str!("../../../wprs-editor/src/molds/media.js");
    let events = ["mediaStatus", "mediaPlayback", "mediaProperties", "mediaThumbnail", "mediaTimeline"];
    let text = json!({ "script": "scripts/media.js", "fn": "nowPlaying" });
    let scene = json!({ "scripts": [{ "path": "scripts/media.js", "events": events, "uses": ["media"] }],
        "layers": [{ "id": "line", "type": "text", "text": text, "size": 10 }] });
    let mut script = staged(scene, &[("scripts/media.js", source)]);
    let one = track("One", true, 2);
    let out = script.update(&Frame { media: Some((&one, 0.0)), ..Frame::at(0.0) }).unwrap();
    let key = script_key("scripts/media.js", Some("nowPlaying"), None);
    assert_eq!(out.values[&key], json!("One · Example Artist"));
}
