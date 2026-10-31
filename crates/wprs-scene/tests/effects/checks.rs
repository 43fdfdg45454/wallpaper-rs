use super::*;

#[test]
fn lut_files_parse_within_limits() {
    let corners = (0..8).map(|i| format!("{} {} {}\n", i & 1, (i >> 1) & 1, i >> 2)).collect::<String>();
    let head = "TITLE \"probe\"\n# comment\nLUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\n";
    let lut = Lut::parse(format!("{head}{corners}").as_bytes()).unwrap();
    assert_eq!(
        (lut.size, lut.data.len(), lut.data[1], lut.data[6]),
        (2, 8, [1.0, 0.0, 0.0], [0.0, 1.0, 1.0])
    );
    let short: String = corners.lines().take(7).map(|l| format!("{l}\n")).collect();
    let cases = [
        (corners.clone(), LutError::Size),
        (format!("LUT_3D_SIZE 1\n{corners}"), LutError::Size),
        (format!("LUT_3D_SIZE 66\n{corners}"), LutError::Size),
        (format!("LUT_3D_SIZE 2\n{short}"), LutError::Count(7, 8)),
        (format!("LUT_3D_SIZE 2\n{corners}0 0 0\n"), LutError::Count(9, 8)),
        (format!("LUT_1D_SIZE 4\n{corners}"), LutError::Line(1)),
        (format!("LUT_3D_SIZE 2\nDOMAIN_MAX 2 2 2\n{corners}"), LutError::Line(2)),
        (format!("LUT_3D_SIZE 2\nNaN 0 0\n{short}"), LutError::Line(2)),
    ];
    for (text, error) in cases {
        assert_eq!(Lut::parse(text.as_bytes()), Err(error), "{text}");
    }
    assert_eq!(Lut::parse(&[0xff, 0xfe]), Err(LutError::Text));
    assert_eq!(Lut::parse(&vec![b' '; MAX_LUT_BYTES + 1]), Err(LutError::TooLarge));
    let es = wprs_core::message::tr_in(&["es_ES.UTF-8".into()]);
    assert_eq!(LutError::Line(2).to_string(), "line 2 of the LUT is not valid");
    assert_eq!(LutError::Count(7, 8).message().told(&es), "la LUT tiene 7 entradas y se esperaban 8");
    let invalid = Scene::from_json(r#"{ "schema": "nope" }"#).unwrap_err().message();
    assert!(invalid.told(&es).starts_with("escena no válida: "), "{}", invalid.told(&es));
}

#[test]
fn descriptors_are_checked_and_tuned_per_quality() {
    let tuned = probe(
        json!({ "id": "probe_tune", "params": [{ "id": "taps", "type": "float", "default": 16 }],
            "passes": [{ "entry": "fs_a" }], "cost": { "taps": 8 },
            "quality": { "q1": { "params": { "taps": 8 } }, "q0": { "params": { "taps": 4 }, "scale": 0.5 } } }),
        "",
    );
    let at = |q| (tuned.tuned(q, "taps").cloned(), tuned.scale_at(q));
    assert_eq!([at(Quality::Q3), at(Quality::Q2)], [(None, 1.0), (None, 1.0)]);
    assert_eq!(
        [at(Quality::Q1), at(Quality::Q0)],
        [(Some(Param::Number(8.0)), 1.0), (Some(Param::Number(4.0)), 0.5)]
    );
    assert_eq!(descriptor("probe_tune"), Some(tuned));
    assert!(register(tuned.clone()).is_ok());
    assert!(register(Descriptor { cost: Cost::default(), ..tuned.clone() }).is_err());
    assert!(register(Descriptor { wgsl: "changed".into(), ..descriptor("blur").unwrap().clone() }).is_err());
    let rt = runtime(json!([{ "effect": "probe_tune", "params": { "taps": 32 } }]));
    let slots = |q| rt.frame(VIEW, q).effects.first().map(|e| (e.slots[0][0], e.scale));
    assert_eq!(
        [slots(Quality::Q3), slots(Quality::Q2), slots(Quality::Q0)],
        [Some((32.0, 1.0)), Some((32.0, 0.5)), None]
    );
    let broken = [
        (json!({ "passes": [{ "entry": "fs_a", "inputs": ["pass:0"] }] }), "unknown input pass:0"),
        (json!({ "passes": [{ "entry": "fs_a", "output": "state" }] }), "missing state"),
        (json!({ "passes": [{ "entry": "fs_a", "inputs": ["asset:map"] }] }), "unknown input asset:map"),
        (json!({ "fuse": "uv" }), "fn uv_probe_bad("),
        (json!({ "passes": [{ "entry": "fs_a" }], "reach": "size" }), "reach"),
        (json!({ "passes": [], "params": [{ "id": "k", "type": "texture", "default": 1 }] }), "default of k"),
        (json!({ "passes": [{ "entry": "fs_a" }], "quality": { "q0": { "params": { "k": 1 } } } }), "sets k"),
    ];
    for (mut json, problem) in broken {
        json["id"] = json!("probe_bad");
        let d = Descriptor::parse(&json.to_string(), "").unwrap();
        assert!(d.problems().iter().any(|p| p.contains(problem)), "{problem}: {:?}", d.problems());
        assert!(register(d).is_err());
    }
}
