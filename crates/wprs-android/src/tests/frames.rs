use super::ask;
use crate::{choice, depth, framing, library, power, store, tilt};
use serde_json::{Value, json};

pub(super) fn framing(green: &str, bands: &str) {
    let deep = |request: Value| serde_json::from_str::<Value>(&depth::request(&request.to_string())).unwrap();
    let image = |uid: &str| library::with(|lib| Ok(lib.item(uid)?.path)).unwrap();
    ask(json!({ "op": "pick", "choice": { "item": green } }));
    choice::commit(true, choice::Target::Both).unwrap();
    let wanted = || [tilt::wanted(true, false), tilt::wanted(false, false)];
    assert_eq!(wanted(), [false, false]);
    let fresh = deep(json!({ "op": "item", "uid": green }));
    assert_eq!(
        (&fresh["strength"], &fresh["fit"], &fresh["crop"], &fresh["base"], &fresh["paused"]),
        (&json!(0.02f32), &Value::Null, &Value::Null, &Value::Null, &Value::Null)
    );
    assert_eq!(deep(json!({ "op": "parallax", "uid": green, "on": true })), Value::Null);
    assert_eq!(wanted(), [true, true]);
    let saved = depth::loaded(&image(green)).unwrap().parallax.unwrap();
    assert_eq!(saved.source, wprs_core::edit::Motion::Auto, "the phone leaves the source to each device");
    ask(json!({ "op": "pick", "choice": { "item": bands } }));
    assert_eq!(
        wanted(),
        [false, true],
        "the preview follows the item it shows, the wallpaper the chosen one"
    );
    ask(json!({ "op": "pick", "choice": null }));
    let power = |f: fn(&mut power::Power)| {
        let mut state = power::Power {
            charging: false,
            saver: false,
            low: false,
            thermal: 0,
            headroom: 0.0,
            still: false,
        };
        f(&mut state);
        [tilt::rate(true, state, false), tilt::rate(true, state, true)]
    };
    assert_eq!(
        [power(|_| {}), power(|p| p.charging = true), power(|p| p.saver = true)],
        [[30, 30], [60, 30], [0, 0]]
    );
    assert_eq!([power(|p| p.low = true), power(|p| (p.low, p.charging) = (true, true))], [[0, 0], [60, 30]]);
    assert_eq!(tilt::rate(false, power::Power::now(), false), 0);
    deep(json!({ "op": "parallax", "uid": green, "strength": 0.03, "focus": 0.5 }));
    let tuned = deep(json!({ "op": "item", "uid": green }));
    let strength = tuned["strength"].as_f64().unwrap();
    assert!(
        tuned["parallax"] == json!(true) && (strength - 0.03).abs() < 1e-6 && tuned["focus"] == json!(0.5)
    );
    assert!(
        deep(json!({ "op": "parallax", "uid": green, "strength": 0.5 }))["error"]
            .as_str()
            .unwrap()
            .contains("strength")
    );
    let moving = || {
        let item = deep(json!({ "op": "item", "uid": green }));
        (item["parallax"].clone(), item["strength"].as_f64().unwrap() as f32)
    };
    deep(json!({ "op": "parallax", "uid": green, "strength": 0.0 }));
    assert_eq!(moving(), (json!(false), 0.0));
    deep(json!({ "op": "parallax", "uid": green, "on": true }));
    assert_eq!(moving(), (json!(true), 0.02), "turning it on again gives it back a strength");
    assert_eq!(deep(json!({ "op": "fit", "uids": [green, bands, "missing"], "fit": "center" })), json!(2));
    assert_eq!(deep(json!({ "op": "item", "uid": bands }))["fit"], json!("center"));
    let framed = |preview| framing::framed(green, &image(green), wprs_core::Fit::Fill, preview);
    assert_eq!((framed(false).fit, framed(false).crop), (wprs_core::Fit::Center, None));
    let generation = store::generation();
    let frame = |pan: [f32; 2], zoom: f32, done: bool| {
        let request = json!({ "op": "frame", "uid": green, "pan": pan, "zoom": zoom, "screen": [32, 64], "done": done });
        serde_json::from_value::<[f32; 4]>(deep(request)).unwrap()
    };
    let zoomed = frame([0.0, 0.0], 2.0, false);
    assert!(store::generation() > generation && framing::draft(green) == Some(zoomed));
    assert_eq!(zoomed, [0.4375, 0.25, 0.125, 0.5]);
    assert_eq!((framed(true).crop, framed(false).crop), (Some(zoomed), None));
    let generation = store::generation();
    let moved = frame([0.5, 0.0], 1.0, true);
    assert_eq!(moved, [0.375, 0.25, 0.125, 0.5]);
    assert!(store::generation() > generation && framing::draft(green).is_none());
    assert_eq!(deep(json!({ "op": "item", "uid": green }))["crop"], json!(moved));
    let edit = depth::loaded(&image(green)).unwrap();
    assert_eq!(
        (edit.geometry.crop, edit.outputs[framing::SCREEN].crop),
        (None, moved),
        "the phone keeps its framing as its own screen, not as the crop every screen uses"
    );
    deep(json!({ "op": "fit", "uids": [green], "fit": null }));
    assert_eq!(
        (framed(false).fit, framed(false).crop, framed(false).develop.is_none()),
        (wprs_core::Fit::Fill, Some(moved), true)
    );
    let crop = |crop| {
        depth::change(&image(green), |edit| {
            edit.geometry.crop = crop;
            Ok(())
        })
        .unwrap()
    };
    crop(Some([0.0, 0.0, 0.5, 1.0]));
    assert_eq!(
        framed(false).crop,
        Some([0.1875, 0.25, 0.0625, 0.5]),
        "the framing sits inside a shared crop"
    );
    frame([0.0, 0.0], 1.0, false);
    ask(json!({ "op": "unpick", "choice": { "item": green } }));
    assert_eq!(framing::draft(green), None, "leaving the page drops an unfinished framing");
    let fill = |crop, pan| framing::placed(wprs_core::Fit::Fill, crop, (64, 32), (32, 32), pan);
    assert_eq!(
        (fill(Some(moved), 0.5).offset, fill(Some(moved), 0.5).scale),
        ([0.375, 0.375], [0.125, 0.25])
    );
    assert_eq!([fill(None, 0.0).offset, fill(None, 1.0).offset], [[0.0, 0.0], [0.5, 0.0]]);
    assert_eq!(
        fill(Some([0.0, 0.0, 1.0, 0.5]), 1.0).offset,
        [0.75, 0.0],
        "the launcher scrolls inside a crop wider than the screen"
    );
    let phone = |pan| framing::placed(wprs_core::Fit::Fill, Some(zoomed), (64, 32), (32, 64), pan).offset;
    assert_eq!(phone(0.0), phone(1.0), "a framing in the shape of the screen stays still");
    assert_eq!(deep(json!({ "op": "unframe", "uid": green })), Value::Null);
    let unframed = deep(json!({ "op": "item", "uid": green }));
    assert_eq!((&unframed["crop"], &unframed["base"]), (&Value::Null, &json!([0.0, 0.0, 0.5, 1.0])));
    crop(None);
    deep(json!({ "op": "fit", "uids": [bands], "fit": null }));
    deep(json!({ "op": "parallax", "uid": green, "on": false }));
    assert_eq!(wanted(), [false, false]);
    assert!(deep(json!({ "op": "make", "uid": green }))["error"].as_str().unwrap().contains("not installed"));
    assert_eq!(ask(json!({ "op": "favorite", "uids": [green, "gone"], "favorite": true })), Value::Null);
    assert!(
        library::with(|lib| Ok(lib.item(green)?.favorite)).unwrap(),
        "a missing item does not stop the rest"
    );
}
