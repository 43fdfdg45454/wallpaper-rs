use super::{act, ask, edit, layer, project, undone};
use serde_json::json;

#[test]
fn camera_paths_take_keys_from_the_view_and_show_their_spline() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    let before = p.doc.clone();
    edit(&mut p, json!({ "path": { "add": { "id": "tour" } } })).unwrap();
    undone(&mut p, &before);
    edit(&mut p, json!({ "view": { "orbit": [40, 10] } })).unwrap();
    edit(&mut p, json!({ "path": { "key": { "path": "tour", "time": 4, "from_view": true } } })).unwrap();
    edit(&mut p, json!({ "path": { "key": { "path": "tour", "time": 1, "eye": [2, 0, 5], "center": [0, 0, 0], "fov": 40 } } })).unwrap();
    edit(&mut p, json!({ "path": { "key": { "path": "tour", "time": 3, "from": 1 } } })).unwrap();
    let keys = p.doc.pointer("/scene/camera3d/paths/0/keys").unwrap().as_array().unwrap().clone();
    assert_eq!(keys.iter().map(|k| k["t"].as_f64().unwrap()).collect::<Vec<_>>(), [0.0, 2.0, 3.0, 4.0]);
    assert_eq!(keys[2]["fov"], json!(40.0));
    edit(&mut p, json!({ "path": { "set": { "path": "tour", "loop": true, "ease": "in_out_sine" } } }))
        .unwrap();
    edit(&mut p, json!({ "path": { "order": { "order": "random" } } })).unwrap();
    edit(&mut p, json!({ "path": { "fade": { "to": "cross", "duration": 0.5 } } })).unwrap();
    let cam = p.doc.pointer("/scene/camera3d").unwrap();
    assert_eq!((cam["order"].clone(), cam["fade"]["to"].clone()), (json!("random"), json!("cross")));
    let shown = ask(&p, json!({ "paths3d": { "time": 1.0 } }));
    let tour = &shown["paths"][0];
    assert_eq!(tour["keys"].as_array().unwrap().len(), 4);
    assert!(tour["spline"].as_array().unwrap().len() > 30, "{tour}");
    assert!(shown["playing"]["moving"] == true);
    for bad in [
        json!({ "key": { "path": "tour", "time": 0, "fov": 400 } }),
        json!({ "add": { "id": "tour" } }),
        json!({ "key": { "path": "nothing", "time": 0 } }),
        json!({ "order": { "order": "shuffled" } }),
        json!({ "fade": { "duration": 20 } }),
        json!({ "set": { "path": "tour", "ease": "wobbly" } }),
    ] {
        assert!(edit(&mut p, json!({ "path": bad.clone() })).is_err(), "{bad}");
    }
    edit(&mut p, json!({ "path": { "key": { "path": "tour", "time": 3, "remove": true } } })).unwrap();
    edit(&mut p, json!({ "path": { "key": { "path": "tour", "time": 2, "remove": true } } })).unwrap();
    assert!(
        edit(&mut p, json!({ "path": { "key": { "path": "tour", "time": 0, "remove": true } } })).is_err(),
        "two keys stay"
    );
    edit(&mut p, json!({ "path": { "remove": { "path": "tour" } } })).unwrap();
    assert!(p.doc.pointer("/scene/camera3d/paths").is_none());
}

#[test]
fn shadows_are_switched_sized_biased_and_shown_per_tier() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    act(&mut p, json!({ "add_layer": { "kind": "light", "id": "sun" } })).unwrap();
    let before = p.doc.clone();
    edit(&mut p, json!({ "shadows": { "layer": "sun", "on": true, "resolution": 1024, "bias": 0.4 } }))
        .unwrap();
    undone(&mut p, &before);
    let sun = layer(&p, "sun");
    assert_eq!((sun["shadows"].clone(), sun["shadow_resolution"].clone()), (json!(true), json!(1024)));
    let shown = ask(&p, json!({ "shadows3d": { "time": 0 } }));
    let tiers = &shown["lights"][0]["tiers"];
    assert_eq!(tiers[0]["side"], 0);
    assert_eq!(tiers[3]["side"], 1024);
    assert_eq!(tiers[3]["low"], 512);
    assert!(!shown["lights"][0]["frustum"].as_array().unwrap().is_empty());
    edit(&mut p, json!({ "shadows": { "layer": "sun", "automatic": true } })).unwrap();
    assert!(layer(&p, "sun").get("shadow_resolution").is_none());
    for bad in [
        json!({ "layer": "sun", "resolution": 100 }),
        json!({ "layer": "sun", "bias": 3 }),
        json!({ "layer": "doll", "on": true }),
    ] {
        assert!(edit(&mut p, json!({ "shadows": bad.clone() })).is_err(), "{bad}");
    }
    edit(&mut p, json!({ "material": { "layer": "doll", "name": "paint", "cast_shadows": false, "receive_shadows": true } })).unwrap();
    edit(&mut p, json!({ "debug": { "show": "shadows" } })).unwrap();
    assert_eq!(p.doc.pointer("/scene/ext/editor/debug3d"), Some(&json!("shadows")));
    assert!(edit(&mut p, json!({ "debug": { "show": "bones" } })).is_err());
    edit(&mut p, json!({ "debug": {} })).unwrap();
    edit(&mut p, json!({ "shadows": { "layer": "sun", "on": false } })).unwrap();
    assert!(layer(&p, "sun").get("shadows").is_none());
}

#[test]
fn material_shaders_take_their_params_and_package_ones_report_their_lines() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project(&root);
    edit(&mut p, json!({ "model": { "add": { "asset": "toy", "id": "doll" } } })).unwrap();
    let fur = json!({ "layer": "doll", "name": "paint", "shader": "fur", "params": { "layers": 16, "length": 20, "comb_x": 0.5 } });
    edit(&mut p, json!({ "material": fur })).unwrap();
    assert!(
        edit(&mut p, json!({ "material": { "layer": "doll", "name": "paint", "params": { "layers": 99 } } }))
            .is_err()
    );
    let chroma = json!({ "layer": "doll", "name": "trim", "shader": "chroma", "key": "#00ff00", "params": { "threshold": 0.3 } });
    edit(&mut p, json!({ "material": chroma })).unwrap();
    assert_eq!(layer(&p, "doll")["materials"]["trim"]["params"]["key"], "#00ff00");
    assert!(
        edit(&mut p, json!({ "material": { "layer": "doll", "name": "paint", "key": "#00ff00" } })).is_err()
    );
    let before = p.doc.clone();
    edit(&mut p, json!({ "new_material": { "name": "glaze_coat" } })).unwrap();
    undone(&mut p, &before);
    let list = ask(&p, json!({ "materials3d": { "layer": "doll" } }));
    assert!(list["packages"].as_array().unwrap().contains(&json!("glaze_coat")), "{}", list["packages"]);
    assert!(list["shaders"].as_array().unwrap().contains(&json!("foliage")));
    let ok = ask(&p, json!({ "material_check": { "name": "glaze_coat" } }));
    assert_eq!((ok["ok"].clone(), ok["material"].clone()), (json!(true), json!(true)), "{ok}");
    std::fs::write(
        p.dir.join("effects/glaze_coat.wgsl"),
        "fn material(s: Surface) -> Surface {\n    var o = s;\n    o.albedo = nope;\n    return o;\n}\n",
    )
    .unwrap();
    let broken = ask(&p, json!({ "material_check": { "name": "glaze_coat" } }));
    assert_eq!(broken["ok"], false);
    assert_eq!(broken["line"], 3, "{broken}");
    assert!(edit(&mut p, json!({ "new_material": { "name": "glaze_coat" } })).is_err(), "taken");
    let cost = ask(&p, json!({ "cost3d": { "quality": "q3" } }));
    let levels = cost["levels"].as_array().unwrap();
    assert!(levels[3]["fur_shells"].as_u64().unwrap() > levels[0]["fur_shells"].as_u64().unwrap());
    assert_eq!(cost["tiers"][2]["shadows"], 1);
}
