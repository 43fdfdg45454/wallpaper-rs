use serde_json::{Value, json};
use wprs_core::edit::{self, ACTIONS, Action, Edit, Error, Kind, Mask, Source};

fn act(edit: &mut Edit, action: Value) -> Result<(), Error> {
    edit.act(serde_json::from_value(action).unwrap())
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
}

fn picture() -> Edit {
    Edit { source: Source { pixels: [4000, 2000], ..Source::default() }, ..Edit::default() }
}

#[test]
fn every_action_is_named_and_takes_its_fields_as_json() {
    for name in ACTIONS {
        let parsed = serde_json::from_value::<Action>(json!({name: {}}));
        assert!(parsed.is_ok() || parsed.unwrap_err().to_string().contains("missing field"), "{name}");
    }
    assert!(serde_json::from_value::<Action>(json!({"blur": {"amount": 1}})).is_err());
    assert!(serde_json::from_value::<Action>(json!({"sharpen": {}})).is_err());
}

#[test]
fn actions_keep_what_they_are_not_given() {
    let mut e = picture();
    act(&mut e, json!({"crop": {"aspect": "1:1"}})).unwrap();
    assert!(near(e.geometry.crop.unwrap(), [0.25, 0.0, 0.5, 1.0]));
    act(&mut e, json!({"crop": {"rotate": 90, "aspect": "1:1"}})).unwrap();
    assert_eq!((e.geometry.rotate, e.geometry.frame((4000, 2000))), (90, (2000, 2000)));
    act(&mut e, json!({"crop": {"rect": "0,0,0.5,0.5", "flip": "v", "output": "DP-1", "aspect": "16:9"}}))
        .unwrap();
    assert!(near(e.outputs["DP-1"].crop, [0.0, 0.0625, 0.6667, 0.375]), "{:?}", e.outputs);
    assert_eq!((e.geometry.flip, e.geometry.rotate), ([false, true], 90));
    act(&mut e, json!({"crop": {"rect": "none", "output": "DP-1"}})).unwrap();
    assert!(e.outputs.is_empty());
    act(&mut e, json!({"vignette": {"amount": -0.4}})).unwrap();
    act(&mut e, json!({"vignette": {"feather": 0.8}})).unwrap();
    let vignette = Kind::Vignette { amount: -0.4, midpoint: 0.5, roundness: 0.0, feather: 0.8 };
    assert_eq!(e.ops[0].kind, vignette);
    act(&mut e, json!({"overlay": {"blend": "soft-light", "mask": "radial:0.5,0.5,0.2,0.2"}})).unwrap();
    act(&mut e, json!({"overlay": {"color": "#ff8800", "opacity": 0.3}})).unwrap();
    assert_eq!((e.ops[1].mask.as_deref(), e.ops[1].opacity), (Some("ov"), Some(0.3)));
    assert_eq!(e.masks["ov"].spec().as_deref(), Some("radial:0.5,0.5,0.2,0.2,0.5"));
    act(&mut e, json!({"curve": {"points": ["1,1", "0,0.25", "0.5,0.75"], "channel": "g"}})).unwrap();
    let Kind::Curves { g, rgb, .. } = &e.ops[2].kind else { panic!("{:?}", e.ops[2]) };
    assert_eq!((g.as_slice(), rgb.len()), (&[[0.0, 0.25], [0.5, 0.75], [1.0, 1.0]][..], 0));
    assert_eq!(e.get("ops.cu.g"), Some(json!([[0.0, 0.25], [0.5, 0.75], [1.0, 1.0]])));
    assert_eq!((e.get("temperature"), e.get("nothing")), (Some(json!(6504.0)), None));
    act(&mut e, json!({"order": {"ids": ["cu", "ov"]}})).unwrap();
    let ids: Vec<&str> = e.ops.iter().map(|op| op.id.as_str()).collect();
    assert_eq!(ids, ["cu", "ov", "vi"]);
    act(&mut e, json!({"reset": {"ids": ["vi"]}})).unwrap();
    act(&mut e, json!({"span": {"center": "0.4,0.5"}})).unwrap();
    assert_eq!(e.span.as_ref().map(|s| (s.center, s.zoom)), Some(([0.4, 0.5], 1.0)));
    act(&mut e, json!({"reset": {}})).unwrap();
    assert!(e.is_empty() && e.source.pixels == [4000, 2000]);
}

#[test]
fn mistakes_are_refused_without_changing_anything() {
    let mut e = picture();
    let fails = |e: &mut Edit, action: Value| act(e, action).unwrap_err();
    let usage = fails(&mut e, json!({"set": {"changes": ["ev"]}}));
    assert!(matches!(&usage, Error::Usage(m) if m.id == "edit.key_value" && m.args == ["ev"]), "{usage:?}");
    assert_eq!(usage.to_string(), "ev is not KEY=VALUE");
    let es = wprs_core::message::tr_in(&["es".into()]);
    assert_eq!(usage.message().told(&es), "ev no es CLAVE=VALOR");
    let range = Edit::parse(r#"{"version": 1, "span": {"zoom": 9}}"#).unwrap_err();
    assert_eq!(
        (range.id, range.told(&es).as_str()),
        ("edit.range", "span.zoom: 9 está fuera del rango de 1 a 8")
    );
    assert_eq!(
        fails(&mut e, json!({"crop": {"aspect": "wide"}})).to_string(),
        "wide is not a ratio such as 16:9"
    );
    assert!(matches!(fails(&mut e, json!({"mask": {"name": "m", "spec": "square:1"}})), Error::Usage(_)));
    assert!(matches!(fails(&mut e, json!({"order": {"ids": ["nope"]}})), Error::Invalid(_)));
    assert!(matches!(fails(&mut e, json!({"blur": {"radius": 500}})), Error::Invalid(_)));
    assert_eq!(fails(&mut e, json!({"curve": {"channel": "x"}})).to_string(), "x is not rgb, r, g or b");
    assert_eq!(e, picture());
    let spec = Mask::parse("linear:0.5,0,0.5,0.45", true).unwrap();
    assert_eq!(
        (spec.spec().as_deref(), edit::spline(&[[0.0, 0.0], [1.0, 1.0]])(0.25)),
        (Some("linear:0.5,0,0.5,0.45"), 0.25)
    );
}

#[test]
fn empty_edits_remove_their_file() {
    let dir = tempfile::tempdir().unwrap();
    let (image, edits) = (dir.path().join("hills.jpg"), dir.path().join("edits"));
    std::fs::write(&image, b"abc").unwrap();
    let source = Source::read(&image, [3, 1]).unwrap();
    assert_eq!((source.name.as_str(), source.bytes), ("hills.jpg", 3));
    assert_eq!(source.sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let mut e = Edit { source, ..Edit::default() };
    e.set("ev", "1").unwrap();
    assert!(edit::commit(&e, &image, &edits).unwrap().is_some_and(|p| p.exists()));
    act(&mut e, json!({"reset": {"ids": ["ev"]}})).unwrap();
    assert_eq!(edit::commit(&e, &image, &edits).unwrap(), None);
    assert_eq!(edit::load(&image, &edits).unwrap(), None);
}

#[test]
fn a_picture_keeps_its_own_fit() {
    let mut e = picture();
    act(&mut e, json!({"fit": {"mode": "center"}})).unwrap();
    assert_eq!((e.fit, e.is_empty(), e.develops()), (Some(wprs_core::Fit::Center), false, false));
    let saved = Edit::parse(&e.to_json()).unwrap();
    assert_eq!(saved.fit, Some(wprs_core::Fit::Center));
    assert!(matches!(act(&mut e, json!({"fit": {"mode": "span"}})), Err(Error::Invalid(_))));
    assert!(matches!(act(&mut e, json!({"fit": {"mode": "zoom"}})), Err(Error::Usage(_))));
    act(&mut e, json!({"fit": {"mode": "none"}})).unwrap();
    assert!(e.fit.is_none() && e.is_empty());
    assert!(Edit::parse(r#"{"version": 1, "fit": "span"}"#).is_err());
}

#[test]
fn any_name_can_be_a_mask_and_ids_keep_their_kind() {
    let mut e = picture();
    for name in ["1", "true", "null", "sky"] {
        act(&mut e, json!({"mask": {"name": name, "spec": "linear:0,0,1,1"}})).unwrap();
        act(&mut e, json!({"blur": {"radius": 4}})).unwrap();
        e.set("ops.bl.mask", name).unwrap();
        assert_eq!(e.ops[0].mask.as_deref(), Some(name));
    }
    e.set("ops.bl.mask", "").unwrap();
    assert_eq!(e.ops[0].mask, None);
    e.set("ops.bl.mask", "\"true\"").unwrap();
    assert_eq!(e.ops[0].mask.as_deref(), Some("true"));
    let before = e.clone();
    let refused = act(&mut e, json!({"curve": {"points": ["0,0", "1,1"], "id": "bl"}})).unwrap_err();
    assert_eq!(refused.to_string(), "bl is a blur operation, not curves; give another id");
    assert!(act(&mut e, json!({"vignette": {"amount": 0.2, "id": "bl"}})).is_err());
    assert!(act(&mut e, json!({"overlay": {"id": "bl"}})).is_err());
    act(&mut e, json!({"curve": {"points": ["0,0", "1,1"]}})).unwrap();
    assert!(act(&mut e, json!({"blur": {"radius": 2, "id": "cu"}})).is_err());
    assert_eq!(e.ops.len(), before.ops.len() + 1);
}

#[test]
fn brush_strokes_are_painted_and_erased_one_stroke_at_a_time() {
    let mut e = picture();
    act(&mut e, json!({"mask": {"name": "face", "spec": "brush:0.04,0.6:0.41,0.33;0.43,0.35"}})).unwrap();
    act(&mut e, json!({"mask": {"name": "face", "spec": "erase:0.02,1:0.42,0.34"}})).unwrap();
    let Mask::Brush { strokes, invert } = &e.masks["face"] else { panic!("{:?}", e.masks) };
    assert_eq!((strokes.len(), *invert, strokes[0].points.len()), (2, false, 2));
    assert!(!strokes[0].erase && strokes[1].erase && strokes[1].hardness == 1.0);
    act(&mut e, json!({"mask": {"name": "face", "spec": "brush:0.05,0.5:0.4,0.4", "invert": true}})).unwrap();
    act(&mut e, json!({"mask": {"name": "face", "spec": "erase:0.02,1:0.4,0.4"}})).unwrap();
    let Mask::Brush { strokes, invert } = &e.masks["face"] else { panic!("{:?}", e.masks) };
    assert_eq!((strokes.len(), *invert), (4, true));
    let before = e.clone();
    assert!(matches!(
        act(&mut e, json!({"mask": {"name": "face", "spec": "brush:0.04:0.4,0.3"}})),
        Err(Error::Usage(_))
    ));
    assert!(act(&mut e, json!({"mask": {"name": "face", "spec": "brush:0.9,0.5:0.4,0.3"}})).is_err());
    assert_eq!(e, before);
    act(&mut e, json!({"mask": {"name": "face", "spec": "none"}})).unwrap();
    assert!(e.masks.is_empty());
}
