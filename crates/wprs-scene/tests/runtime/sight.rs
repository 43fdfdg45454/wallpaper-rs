use super::*;

fn filmed(layers: serde_json::Value) -> Runtime {
    runtime(
        &json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers,
            "assets": { "clip": { "path": "clip.webm" } } })
        .to_string(),
    )
}

fn rolled(rt: &mut Runtime) -> (f64, bool, bool) {
    rt.frame(VIEW, Quality::Q3);
    rt.advance(0.5);
    let moving = rt.frame(VIEW, Quality::Q3).draws.iter().any(|d| d.dynamic);
    (rt.tapes()[0].at, rt.tapes()[0].shown, moving)
}

#[test]
fn a_hidden_video_that_masks_a_shown_layer_keeps_playing() {
    let video = json!({ "id": "v", "type": "video", "video": "clip", "visible": false });
    let reader = |visible: bool| {
        json!({ "id": "red", "type": "solid", "color": "#ff0000", "size": [50, 50],
            "mask": { "layer": "v" }, "visible": visible })
    };
    let (at, shown, moving) = rolled(&mut filmed(json!([video, reader(true)])));
    assert!(shown && at > 0.4 && moving, "{at} {shown} {moving}");
    let mut unseen = filmed(json!([video, reader(false)]));
    unseen.frame(VIEW, Quality::Q3);
    let (at, shown, moving) = rolled(&mut unseen);
    assert!(!shown && at == 0.0 && !moving, "{at} {shown} {moving}");
}

#[test]
fn a_hidden_video_read_by_an_effect_of_a_shown_layer_keeps_playing() {
    let video = json!({ "id": "v", "type": "video", "video": "clip", "visible": false });
    let reader = json!({ "id": "deep", "type": "solid", "color": "#ff0000", "size": [50, 50],
        "effects": [{ "effect": "depth_parallax", "params": { "depth": { "layer": "v" } } }] });
    let (at, shown, moving) = rolled(&mut filmed(json!([video, reader])));
    assert!(shown && at > 0.4 && moving, "{at} {shown} {moving}");
}

#[test]
fn a_video_runs_at_a_rate_its_decoder_plays() {
    for (asked, played) in [(8.0, 4.0), (0.05, 0.1), (0.0, 0.0), (2.0, 2.0)] {
        let video = json!({ "id": "v", "type": "video", "video": "clip", "playback": { "rate": asked } });
        let mut rt = filmed(json!([video]));
        rt.frame(VIEW, Quality::Q3);
        rt.advance(0.25);
        let t = &rt.tapes()[0];
        assert!(t.rate == played && close(t.at as f32, 0.25 * played), "{asked}: {} {}", t.rate, t.at);
    }
}
