use super::*;

fn timed(timelines: Value, layers: Value) -> Value {
    json!({ "canvas": canvas("#000000"), "timelines": timelines, "layers": layers })
}

#[test]
fn wrapped_timelines_and_events_move_what_is_drawn() {
    let Some(r) = renderer() else { return };
    let fade = json!([{ "id": "f", "type": "solid", "color": "#ffffff", "size": [100, 50],
        "opacity": { "timeline": "w", "keys": [[0, 1], [2, 0]] } }]);
    let mut wrapped = Shot::with(
        r,
        timed(json!([{ "id": "w", "duration": 4, "loop": "repeat", "wrap": true }]), fade),
        &[],
    );
    wrapped.runtime.advance(3.0);
    check(&wrapped.pixels(r), &[((50, 25), [188, 188, 188])]);
    let moved = json!([{ "id": "b", "type": "solid", "color": "#ff0000", "size": [20, 50],
        "transform": { "position": { "timeline": "go", "keys": [[0, [10, 25]], [1, [90, 25]]] } } }]);
    let tls = json!([{ "id": "clock", "duration": 2, "events": [{ "t": 1, "name": "start" }] },
                     { "id": "go", "duration": 1, "trigger": "event:start" }]);
    let mut shot = Shot::with(r, timed(tls, moved), &[]);
    shot.runtime.advance(0.9);
    check(&shot.pixels(r), &[((10, 25), [255, 0, 0]), ((90, 25), [0, 0, 0])]);
    shot.runtime.advance(1.2);
    check(&shot.pixels(r), &[((10, 25), [0, 0, 0]), ((90, 25), [255, 0, 0])]);
}

#[test]
fn scenes_without_a_preview_are_drawn_at_their_poster_time() {
    let Some(r) = renderer() else { return };
    let mut text = timed(
        json!([{ "id": "t", "duration": 4 }]),
        json!([
        { "id": "s", "type": "solid", "size": [100, 50],
          "color": { "timeline": "t", "keys": [[0, "#000000"], [2, "#00ff00"]] } },
        { "id": "noon", "type": "solid", "color": "#ffffff", "size": [20, 50], "transform": { "position": [10, 25] },
          "opacity": { "clock": "day" } }]),
    );
    text["schema"] = json!(SCHEMA);
    text["canvas"]["size"] = json!([200, 100]);
    text["clock"] = json!({ "poster_time": 3 });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let poster = |wall: f64| r.poster(&scene, &SceneAssets::default(), &Default::default(), (wall, "en"), 50);
    let night = poster(0.0).unwrap();
    assert_eq!(night.dimensions(), (50, 25));
    let p = night.get_pixel(25, 12);
    assert!(p[0] < 4 && p[1] > 250 && p[2] < 4 && p[3] == 255, "{p:?}");
    assert!(night.get_pixel(2, 6)[0] < 4, "{:?}", night.get_pixel(2, 6));
    let noon = poster(86_400.0 * 1000.5).unwrap().get_pixel(2, 6)[0];
    assert!((150..225).contains(&noon), "thumbnails show the local time they were drawn at: {noon}");
    let read = SceneAssets::read(&scene, |_| Err("nothing to read".into())).unwrap();
    assert!(read.images.is_empty());
}
