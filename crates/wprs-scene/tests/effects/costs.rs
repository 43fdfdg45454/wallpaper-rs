use super::*;

#[test]
fn asset_parameters_name_assets_of_their_kind() {
    probe(
        json!({ "id": "probe_assets", "passes": [{ "entry": "fs_a", "inputs": ["previous", "asset:depth"] }],
            "params": [{ "id": "depth", "type": "texture", "default": "" }, { "id": "grade", "type": "lut", "default": "" }] }),
        "",
    );
    let issues = |params: serde_json::Value| {
        scene(json!([{ "effect": "probe_assets", "params": params }])).validate(&BTreeMap::new())
    };
    assert_eq!(issues(json!({ "depth": "a", "grade": "grade" })), []);
    let path = |p: &str| format!("/layers/0/effects/0/params/{p}");
    let unknown = Issue::Unknown { path: path("depth"), what: "asset", id: "gone".into() };
    assert_eq!(issues(json!({ "depth": "gone" })), [unknown]);
    assert_eq!(
        issues(json!({ "grade": "a", "depth": "grade" })),
        [Issue::Type(path("depth")), Issue::Type(path("grade"))]
    );
    assert_eq!(issues(json!({ "depth": { "prop": "x" } })).last(), Some(&Issue::Type(path("depth"))));
}

#[test]
fn built_in_effects_report_their_cost() {
    for d in builtin() {
        assert!(d.units().is_finite() && d.units() > 0.0, "{}", d.id);
    }
    let class = |id: &str| descriptor(id).map(Descriptor::class);
    let light = ["dissolve", "flow", "iris", "lens_distort", "perspective", "reflection", "scroll", "shake"];
    assert_eq!(light.map(class), [Some(Class::Light); 8]);
    assert_eq!(["sway", "twirl", "water_rings"].map(class), [Some(Class::Light); 3]);
    assert_eq!(["clouds", "water_ripple"].map(class), [Some(Class::Medium); 2]);
    let clouds = descriptor("clouds").unwrap();
    let octaves = |q| clouds.tuned(q, "octaves").cloned();
    assert_eq!(
        [Quality::Q3, Quality::Q2, Quality::Q1].map(octaves),
        [None, Some(Param::Number(4.0)), Some(Param::Number(3.0))]
    );
    let still = ["twirl", "lens_distort", "perspective", "dissolve"];
    assert!(still.iter().all(|id| !descriptor(id).unwrap().animates()));
    assert_eq!(descriptor("shake").unwrap().pack(|_| None)[0][0], 0.0);
}

#[test]
fn perspective_solves_the_homography_of_its_corners() {
    let d = descriptor("perspective").unwrap();
    let corners = [[0.2, 0.1], [0.9, 0.0], [1.0, 1.0], [0.0, 0.8]];
    let names = ["top_left", "top_right", "bottom_right", "bottom_left"];
    let slots = d.pack(|id| names.iter().position(|n| *n == id).map(|i| Param::Vec2(corners[i])));
    let unit = |[x, y]: [f32; 2]| {
        let h = [4, 5, 6].map(|r| slots[r][0] * x + slots[r][1] * y + slots[r][2]);
        [h[0] / h[2], h[1] / h[2]]
    };
    for (corner, expected) in corners.into_iter().zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]) {
        let found = unit(corner);
        assert!(found.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-4), "{found:?} {expected:?}");
    }
    let mut broken = d.clone();
    broken.params.pop();
    assert!(broken.problems().iter().any(|p| p.contains("homography")));
}

#[test]
fn ripples_wake_on_presses_and_settle() {
    let mut rt = runtime(json!([{ "effect": "water_ripple" }]));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    let press = PointerState {
        pos: Some([0.5; 2]),
        smooth: [0.5; 2],
        buttons: 1,
        inside: true,
        presence: 1.0,
        ..Default::default()
    };
    assert!(rt.set_pointer(press) && !press.moving());
    let mut wakes = Vec::new();
    for _ in 0..3 {
        wakes.push(rt.frame(VIEW, Quality::Q3).wake);
        rt.advance(1.9);
    }
    assert_eq!(wakes, [Wake::Frame, Wake::Frame, Wake::Frame]);
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    assert!(rt.set_pointer(PointerState { buttons: 0, ..press }));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Idle);
    assert!(rt.set_pointer(PointerState { buttons: 2, ..press }));
    assert_eq!(rt.frame(VIEW, Quality::Q3).wake, Wake::Frame);
}
