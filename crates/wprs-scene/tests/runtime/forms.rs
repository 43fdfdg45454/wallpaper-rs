use super::*;

#[test]
fn forms_order_localize_and_hide_properties() {
    let mut props = props();
    let extra: BTreeMap<String, Property> = serde_json::from_value(json!({
        "look": { "type": "group", "label": { "en": "Look", "es": "Aspecto" }, "order": -1 },
        "glow": { "type": "slider", "group": "look", "min": 0, "max": 1, "step": 0.1, "default": 0.5,
                  "label": { "en": "Glow", "es-ES": "Brillo" } }
    }))
    .unwrap();
    props.extend(extra);
    let presets: Vec<Preset> = serde_json::from_value(
        json!([{ "id": "calm", "label": { "en": "Calm", "es": "Calma" }, "values": {} }]),
    )
    .unwrap();
    let values = effective(&props, &[&BTreeMap::from([("soft".to_string(), json!(false))])]);
    let (fields, labels) = form(&props, &presets, &values, &["es_ES.UTF-8".to_string()]);
    let keys: Vec<_> = fields.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(
        keys,
        ["look", "glow", "density", "depth", "photo", "show_clock", "soft", "style", "text_color"]
    );
    assert_eq!(
        (fields[0].label.as_str(), fields[1].label.as_str(), &fields[1].value),
        ("Aspecto", "Brillo", &json!(0.5))
    );
    let find = |key: &str| fields.iter().find(|f| f.key == key).unwrap();
    assert!(!find("show_clock").visible && find("soft").visible && find("density").integer);
    let style = find("style");
    let options: Vec<_> =
        style.options.iter().map(|o| (o.value.as_str().unwrap(), o.label.as_str())).collect();
    assert_eq!((style.label.as_str(), options), ("Style", vec![("night", "Night"), ("dawn", "dawn")]));
    assert_eq!(
        (labels[0].label.as_str(), form(&props, &presets, &values, &[]).1[0].label.as_str()),
        ("Calma", "Calm")
    );
    let broken = |extra: serde_json::Value| {
        let mut props = props.clone();
        props.extend(serde_json::from_value::<BTreeMap<String, Property>>(extra).unwrap());
        check_presets(&props, &[]).is_err()
    };
    assert!(!broken(json!({})));
    assert!(broken(json!({ "x": { "type": "bool", "default": true, "group": "soft" } })));
    assert!(broken(json!({ "x": { "type": "bool", "default": true, "visible_if": { "nothing": true } } })));
    assert!(broken(json!({ "x": { "type": "bool", "default": true, "visible_if": { "soft": 3 } } })));
    let mut shown = props.clone();
    shown.extend(
        serde_json::from_value::<BTreeMap<String, Property>>(json!({
            "a": { "type": "bool", "default": true, "visible_if": { "density": 120 } },
            "b": { "type": "bool", "default": true, "visible_if": { "density": 120.0, "text_color": "#FFFFFF" } },
            "c": { "type": "bool", "default": true, "visible_if": { "depth": 0.4 } },
            "d": { "type": "bool", "default": true, "visible_if": { "depth": 1 } }
        }))
        .unwrap(),
    );
    assert_eq!(check_presets(&shown, &[]), Ok(()));
    let fields = form(&shown, &[], &effective(&shown, &[]), &[]).0;
    let visible = |key: &str| fields.iter().any(|f| f.key == key && f.visible);
    assert_eq!(["a", "b", "c", "d"].map(visible), [true, true, true, false]);
}

#[test]
fn declared_inputs_limit_what_a_scene_receives() {
    let layers = json!([{ "id": "glow", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "opacity": { "audio": "bass" }, "parallax": 0.5 }]);
    let mut rt = runtime(&scene(layers));
    assert!(rt.uses().audio && rt.uses().cursor);
    rt.declare(Uses { audio: false, cursor: true, clicks: false, scripts: true, media: false });
    assert!(!rt.uses().audio && rt.uses().cursor && !rt.uses_at(Quality::Q0).audio);
    let loud = AudioFrame { bass: 1.0, silent: false, ..AudioFrame::default() };
    assert!(!rt.set_audio(loud), "an input the package did not declare never wakes it");
}

#[test]
fn a_spare_frame_hands_its_lists_to_the_next_one() {
    let shown = json!([{ "id": "a", "type": "solid", "color": "#ff0000", "size": [10, 10],
        "effects": [{ "effect": "blur", "params": { "radius": 2 } }],
        "transform": { "position": { "keys": [[0, [10, 10]], [2, [90, 40]]] } } }]);
    let mut rt = runtime(&scene(shown));
    let first = rt.frame(VIEW, Quality::Q3);
    let lists = (first.draws.as_ptr() as usize, first.effects.as_ptr() as usize);
    let spare = first.spare();
    rt.advance(0.5);
    let again = rt.frame_in(spare, VIEW, Quality::Q3);
    assert_eq!((again.draws.as_ptr() as usize, again.effects.as_ptr() as usize), lists);
    assert_eq!(again, rt.frame(VIEW, Quality::Q3));
    assert_eq!((again.draws.len(), again.effects.len()), (1, 1));
}
