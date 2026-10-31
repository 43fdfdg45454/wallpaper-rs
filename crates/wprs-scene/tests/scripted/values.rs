use super::*;

#[test]
fn scripts_read_evaluated_layers_and_their_own_writes() {
    let mut rt = runtime(json!({}));
    let set = LayerOverride {
        rotation: Some(30.0),
        size: Some(Extent::Points(20.0)),
        font: Some("nowhere".into()),
        ..LayerOverride::default()
    };
    let mono = LayerOverride {
        font: Some("monospace".into()),
        text: Some("x".repeat(9000)),
        opacity: Some(4.0),
        ..LayerOverride::default()
    };
    rt.set_script(ScriptOutputs {
        layers: [("label".into(), set), ("back".into(), mono)].into(),
        ..ScriptOutputs::default()
    });
    let read = rt.read("label").unwrap();
    assert_eq!(
        (read.rotation, read.size, read.text.as_deref(), read.visible),
        (30.0, Some(Extent::Points(20.0)), Some("a"), true)
    );
    let frame = rt.frame(VIEW, Quality::Q3);
    let text = frame.draws.iter().find_map(|d| match &d.content {
        Content::Text { size, font, .. } => Some((*size, *font)),
        _ => None,
    });
    assert_eq!(text, Some((20.0, Some("serif"))));
    let back = rt.read("back").unwrap();
    assert_eq!((back.opacity, back.size, back.text), (1.0, None, None));
    assert!(rt.read("nothing").is_none());
}

#[test]
fn per_binding_values_are_found_by_their_site() {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] },
        "scripts": [{ "path": "s.js", "params": { "k": { "type": "number", "default": 1 } } }],
        "templates": { "t": { "id": "t", "type": "solid", "color": "#ffffff", "opacity": { "script": "s.js", "fn": "fade" } } },
        "layers": [{ "id": "a", "type": "solid", "color": "#ffffff", "transform": { "rotation": { "script": "s.js", "fn": "turn", "params": { "k": 2 } } },
            "opacity": { "script": "s.js" } }] });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let found = sites(&scene);
    let keys: Vec<(&str, &str, Option<&str>)> =
        found.iter().map(|s| (s.path.as_str(), s.call.as_str(), s.layer.as_deref())).collect();
    assert_eq!(
        keys,
        [("/layers/0/transform/rotation", "turn", Some("a")), ("/templates/t/opacity", "fade", Some("t"))]
    );
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    let values = [(found[0].key.clone(), json!(45.0)), ("s.js".to_string(), json!(0.5))].into();
    rt.set_script(ScriptOutputs { values, ..ScriptOutputs::default() });
    let read = rt.read("a").unwrap();
    assert_eq!((read.rotation, read.opacity), (45.0, 0.5));
    for (bad, path) in [
        (json!({ "script": "s.js", "params": { "k": 2 } }), "/layers/0/opacity/fn"),
        (json!({ "script": "s.js", "fn": "update" }), "/layers/0/opacity/fn"),
    ] {
        let mut text = text.clone();
        text["layers"][0]["opacity"] = bad;
        let error = Scene::load(&text.to_string(), &Default::default()).unwrap_err().to_string();
        assert!(error.contains(path), "{error}");
    }
}

#[test]
fn console_lines_reach_clients_as_plain_text() {
    let rt = runtime(json!({}));
    let console = vec!["log a.js: <b>x</b>\n<img src=\"h\">\u{1b}[31m".to_string(), "x".repeat(600)];
    let stopped = vec!["a.js: Error\r\nmore".to_string()];
    let out = ScriptOutputs { console, stopped, ..ScriptOutputs::default() }.bounded(&rt.scene);
    assert_eq!(out.console[0], "log a.js: <b>x</b> <img src=\"h\"> [31m");
    assert_eq!((out.console[1].len(), out.stopped[0].as_str()), (MAX_LINE, "a.js: Error  more"));
    assert_eq!(plain_line("a\nb\u{7}c"), "a b c");
    assert_eq!(plain_line(&"é".repeat(600)).chars().count(), MAX_LINE);
}
