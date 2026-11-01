use super::*;

fn apply(m: &Value, p: &Value) -> [f64; 2] {
    let m: Vec<f64> = serde_json::from_value(m.clone()).unwrap();
    let [x, y] = [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()];
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

#[test]
fn turbulent_starts_are_set_undone_and_refused_past_their_limits() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    let gust = json!({ "forward": 90, "scale": 0.5, "speed": [20, 40], "audio": "bass" });
    edit(&mut p, "turbulent", json!({ "turbulent": gust })).unwrap();
    assert_eq!(system(&p)["init"]["turbulent"], gust);
    undone(&mut p, &before);
    let before = p.doc.clone();
    edit(&mut p, "turbulent", json!({ "turbulent": { "phase": 2 }, "quiet": true })).unwrap();
    assert_eq!(
        system(&p)["init"]["turbulent"],
        json!({ "forward": 90, "scale": 0.5, "speed": [20, 40], "phase": 2 })
    );
    undone(&mut p, &before);
    let kept = p.doc.clone();
    for bad in
        [json!({ "scale": 3 }), json!({ "speed": [50, 10] }), json!({ "swirl": 1 }), json!({ "audio": 99 })]
    {
        assert!(edit(&mut p, "turbulent", json!({ "turbulent": bad })).is_err(), "{bad} was kept");
        assert_eq!(p.doc, kept);
    }
    edit(&mut p, "turbulent", json!({ "off": true })).unwrap();
    assert!(system(&p)["init"].get("turbulent").is_none());
}

#[test]
fn fractal_offsets_are_set_undone_and_refused_past_their_limits() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let before = p.doc.clone();
    edit(&mut p, "offset", json!({ "offset": { "distance": 12, "octaves": 3, "sign": [0, 1] } })).unwrap();
    assert_eq!(system(&p)["init"]["offset_noise"], json!({ "distance": 12, "octaves": 3, "sign": [0, 1] }));
    undone(&mut p, &before);
    let kept = p.doc.clone();
    for bad in [
        json!({ "octaves": 9 }),
        json!({ "sign": [2, 0] }),
        json!({ "distance": -1 }),
        json!({ "scale": 500 }),
    ] {
        assert!(edit(&mut p, "offset", json!({ "offset": bad })).is_err(), "{bad} was kept");
        assert_eq!(p.doc, kept);
    }
    edit(&mut p, "offset", json!({ "off": true })).unwrap();
    assert!(system(&p)["init"].get("offset_noise").is_none());
}

#[test]
fn the_fan_and_the_offset_arrows_are_what_the_engine_computes() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let gust = json!({ "forward": 30, "scale": 0.25, "noise_speed": 2, "offset": 4, "speed": [10, 80] });
    edit(&mut p, "turbulent", json!({ "turbulent": gust })).unwrap();
    edit(&mut p, "offset", json!({ "offset": { "distance": 20, "octaves": 2, "scale": 3 } })).unwrap();
    let h = wprs_editor::swarms::handles(&p.dir, &p.doc, "flakes", None, 1.5);
    let fan = &h["fields"]["turbulent"];
    assert_eq!((fan["forward"].clone(), fan["fan"].clone()), (json!(30.0), json!([-15.0, 75.0])));
    let samples = fan["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 12);
    for (k, s) in samples.iter().enumerate() {
        let want = wprs_scene::swirl(30.0, 0.25, 3.0 + 4.0 * k as f32 / 12.0);
        assert!((s.as_f64().unwrap() - f64::from(want)).abs() < 0.01, "{k}: {s} {want}");
    }
    assert_eq!(fan["at"], json!([960.0, -20.0]), "the fan starts at the middle of the emitter");
    let drift = &h["fields"]["offset"];
    let arrows = drift["arrows"].as_array().unwrap();
    assert_eq!((arrows.len(), drift["radius"].clone()), (49, json!(20.0)));
    for (k, a) in arrows.iter().enumerate() {
        let local = apply(&h["inverse"], &a[0]).map(|v| v as f32);
        let r = (k as f32 * 0.618_034).fract();
        let q = wprs_scene::fractal(local, [20.0, 3.0, 1.0, 0.0], [0.0, 0.0, 2.0], r).map(|v| json!(v));
        let want = apply(&h["frame"], &json!(q));
        assert!((a[1][0].as_f64().unwrap() - want[0]).abs() < 0.02, "{k}: {a} {want:?}");
        assert!((a[1][1].as_f64().unwrap() - want[1]).abs() < 0.02, "{k}: {a} {want:?}");
    }
}

#[test]
fn anchored_boxes_and_circles_carry_the_inverse_their_handles_need() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, "point", json!({ "point": 1, "at": [100, 50], "angle": 30 })).unwrap();
    let boxed = json!({ "box": { "from": [-10, -5], "to": [10, 5], "at": "p1" } });
    edit(&mut p, "shape", json!({ "emitter": 0, "shape": boxed })).unwrap();
    let ring = json!({ "circle": { "center": [4, 0], "radius": 8, "at": "p1" } });
    edit(&mut p, "shape", json!({ "emitter": 1, "shape": ring })).unwrap();
    let h = handles(&p);
    let (b, c) = (&h["emitters"][0], &h["emitters"][1]);
    let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 0.05 && (a[1] - b[1]).abs() < 0.05;
    assert!(close(apply(&b["inverse"], &b["corners"][0]), [-10.0, -5.0]), "{b}");
    assert!(close(apply(&b["inverse"], &b["corners"][2]), [10.0, 5.0]), "{b}");
    assert!(close(apply(&c["inverse"], &c["center"]), [4.0, 0.0]), "{c}");
    assert!(close(apply(&c["inverse"], &c["edge"]), [12.0, 0.0]), "{c}");
    let (s, k) = 30f64.to_radians().sin_cos();
    assert!(close(apply(&json!([1, 0, 0, 1, 0, 0]), &c["center"]), [100.0 + 4.0 * k, 50.0 + 4.0 * s]), "{c}");
}

#[test]
fn pacing_emitters_evens_the_rope_and_is_refused_for_missing_emitters() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, "shape", json!({ "emitter": 1, "shape": { "point": [10, 10] } })).unwrap();
    edit(&mut p, "strand", json!({ "strand": "rope", "settings": { "width": 4 } })).unwrap();
    assert_eq!(handles(&p)["strands"]["uneven"], json!([0, 1]));
    let before = p.doc.clone();
    edit(&mut p, "pace", json!({ "max_per_step": 1 })).unwrap();
    assert_eq!(
        (
            system(&p)["emitters"][0]["max_per_step"].clone(),
            system(&p)["emitters"][1]["max_per_step"].clone()
        ),
        (json!(1), json!(1))
    );
    assert_eq!(handles(&p)["strands"]["uneven"], json!([]));
    undone(&mut p, &before);
    let kept = p.doc.clone();
    assert!(edit(&mut p, "pace", json!({ "emitter": 9, "max_per_step": 1 })).is_err());
    assert!(edit(&mut p, "pace", json!({ "max_per_step": 0 })).is_err());
    assert!(edit(&mut p, "pace", json!({ "max_per_step": 5000 })).is_err());
    assert_eq!(p.doc, kept);
    edit(&mut p, "pace", json!({ "emitter": 1 })).unwrap();
    assert!(system(&p)["emitters"][1].get("max_per_step").is_none());
    assert_eq!(system(&p)["emitters"][0]["max_per_step"], 1);
}

#[test]
fn systems_whose_id_needs_escaping_are_edited_through_their_layer() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    let s = system(&p);
    act(&mut p, json!({ "set": { "pointer": "/scene/particles/a~1b", "value": s } })).unwrap();
    act(&mut p, json!({ "set": { "pointer": "/scene/layers/1/system", "value": "a/b" } })).unwrap();
    let cut =
        json!({ "particles": { "cutout": { "system": "a/b", "layer": "flakes", "range": [0.2, 0.8, 1] } } });
    act(&mut p, cut).unwrap();
    assert_eq!(p.doc["scene"]["layers"][1]["render"]["cutout"], json!([0.2, 0.8, 1.0]));
}
