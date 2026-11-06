use super::kit::{at, close, runtime};
use serde_json::{Value, json};

fn reached(target: [f32; 2], more: Value) -> [[f32; 2]; 4] {
    let mut ik = json!({ "chain": ["root", "arm", "hand"], "target": [target[0] + 32.0, 32.0 - target[1]] });
    more.as_object().unwrap().iter().for_each(|(k, v)| ik[k] = v.clone());
    let mut rt = runtime(json!({ "ik": [ik] }));
    rt.advance(1.0 / 60.0);
    let p = rt.puppet("fox").unwrap();
    let f = p.bone_frames([64.0, 64.0])[p.rig.joint("hand").unwrap()];
    let tip = [f[0] * 24.0 + f[4] - 32.0, 32.0 - (f[1] * 24.0 + f[5])];
    [at(&rt, "root"), at(&rt, "arm"), at(&rt, "hand"), tip]
}

fn turns(p: &[[f32; 2]; 4]) -> Vec<f32> {
    let angle = |a: [f32; 2], b: [f32; 2]| (b[1] - a[1]).atan2(b[0] - a[0]);
    let wrap = |a: f32| (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    (1..3).map(|i| wrap(angle(p[i], p[i + 1]) - angle(p[i - 1], p[i])).to_degrees()).collect()
}

#[test]
fn a_chain_of_three_bones_curls_the_way_it_is_told_into_an_even_arc() {
    let h = 24.0 * 60f32.to_radians().sin();
    let cw = reached([24.0, 0.0], json!({}));
    for (got, want) in cw.iter().zip([[-24.0, 0.0], [-12.0, h], [12.0, h], [24.0, 0.0]]) {
        assert!(close(*got, want, 0.02), "{cw:?}");
    }
    let ccw = reached([24.0, 0.0], json!({ "bend": "counterclockwise" }));
    for (got, want) in ccw.iter().zip([[-24.0, 0.0], [-12.0, -h], [12.0, -h], [24.0, 0.0]]) {
        assert!(close(*got, want, 0.02), "{ccw:?}");
    }
    assert!(turns(&cw).iter().all(|t| (t + 60.0).abs() < 0.1), "{:?}", turns(&cw));
}

#[test]
fn every_joint_of_a_longer_chain_bends_to_its_side_wherever_the_target_is() {
    for target in [[0.0, 40.0], [-20.0, -30.0], [30.0, 10.0], [-40.0, 0.0], [10.0, -5.0]] {
        for (bend, sign) in [("clockwise", -1.0), ("counterclockwise", 1.0)] {
            let p = reached(target, json!({ "bend": bend }));
            assert!(close(p[3], target, 0.05), "{target:?} {bend}: {p:?}");
            assert!(turns(&p).iter().all(|t| t * sign >= -0.01), "{target:?} {bend}: {:?}", turns(&p));
        }
    }
}

#[test]
fn a_target_out_of_reach_stretches_a_longer_chain_straight_at_it() {
    let p = reached([0.0, 100.0], json!({ "bend": "counterclockwise" }));
    let d = 72.0 / 24f32.hypot(100.0);
    assert!(close(p[3], [-24.0 + 24.0 * d, 100.0 * d], 0.05), "{p:?}");
    let p = reached([0.0, 100.0], json!({ "limits": { "arm": [-10, 10] } }));
    assert!(turns(&p)[0].abs() <= 10.01 && turns(&p).iter().all(|t| *t <= 0.01), "{:?}", turns(&p));
}

fn curled(ik: Option<Value>) -> [[f32; 2]; 4] {
    let bones = json!({ "arm": { "rotation": [[0, 20]] }, "hand": { "rotation": [[0, 50]] } });
    let mut extra =
        json!({ "clips": [{ "id": "curl", "bones": bones }], "animations": [{ "clip": "curl" }] });
    if let Some(ik) = ik {
        extra["ik"] = json!([ik]);
    }
    let mut rt = runtime(extra);
    rt.advance(1.0 / 60.0);
    let p = rt.puppet("fox").unwrap();
    let f = p.bone_frames([64.0, 64.0])[p.rig.joint("hand").unwrap()];
    let tip = [f[0] * 24.0 + f[4] - 32.0, 32.0 - (f[1] * 24.0 + f[5])];
    [at(&rt, "root"), at(&rt, "arm"), at(&rt, "hand"), tip]
}

#[test]
fn a_longer_chain_keeps_the_curl_of_its_clip_while_it_reaches() {
    let pose = curled(None);
    let bent = turns(&pose);
    assert!(bent[0] * bent[1] > 0.0 && (bent[0] - bent[1]).abs() > 10.0, "an uneven curl: {bent:?}");
    let bend = if bent[0] < 0.0 { "clockwise" } else { "counterclockwise" };
    let goal = |to: [f32; 2]| json!({ "chain": ["root", "arm", "hand"], "target": [to[0] + 32.0, 32.0 - to[1]], "bend": bend });
    let held = curled(Some(goal(pose[3])));
    for (got, want) in held.iter().zip(pose) {
        assert!(close(*got, want, 0.05), "a target on the tip leaves the pose: {held:?} {pose:?}");
    }
    let near = [pose[3][0] - 3.0, pose[3][1] + 2.0];
    let reached = curled(Some(goal(near)));
    assert!(close(reached[3], near, 0.05), "{reached:?}");
    let kept = turns(&reached);
    assert!(kept.iter().zip(&bent).all(|(k, b)| (k - b).abs() < 20.0), "{kept:?} keeps {bent:?}");
    assert!((kept[0] - kept[1]).abs() > 5.0, "no even arc: {kept:?}");
}

#[test]
fn a_longer_chain_takes_back_the_curl_of_its_clip_once_its_target_needs_no_arc() {
    let pose = curled(None);
    let bend = if turns(&pose)[0] < 0.0 { "clockwise" } else { "counterclockwise" };
    let spot = |d: [f32; 2]| [pose[3][0] + d[0] + 32.0, 32.0 - pose[3][1] - d[1]];
    let goal = |target: Value| json!({ "chain": ["root", "arm", "hand"], "target": target, "bend": bend });
    let ran = |target: Value, frames: usize| {
        let bones = json!({ "arm": { "rotation": [[0, 20]] }, "hand": { "rotation": [[0, 50]] } });
        let mut rt =
            runtime(json!({ "clips": [{ "id": "curl", "bones": bones }], "animations": [{ "clip": "curl" }],
            "ik": [goal(target)] }));
        (0..frames).for_each(|_| rt.advance(1.0 / 60.0));
        [at(&rt, "root"), at(&rt, "arm"), at(&rt, "hand")]
    };
    let moving =
        json!({ "keys": [[0, spot([5.0, 15.0])], [0.2, spot([5.0, 15.0])], [0.25, spot([-3.0, 2.0])]] });
    let live = ran(moving, 30);
    let fresh = ran(json!(spot([-3.0, 2.0])), 1);
    assert!(
        live.iter().zip(&fresh).all(|(a, b)| close(*a, *b, 0.05)),
        "{live:?} after the arc, {fresh:?} at once"
    );
}

#[test]
fn limits_of_a_two_bone_chain_may_bend_it_against_its_side() {
    for (limits, bend) in [(json!([-150, -5]), "clockwise"), (json!([5, 150]), "counterclockwise")] {
        let ik = json!({ "chain": ["root", "arm"], "target": [-10, 25], "limits": { "arm": limits }, "bend": bend });
        let mut rt = runtime(json!({ "ik": [ik] }));
        rt.advance(1.0 / 60.0);
        assert!(close(at(&rt, "hand"), [-42.0, 7.0], 0.05), "{bend}: {:?}", at(&rt, "hand"));
    }
}

#[test]
fn a_longer_chain_holds_the_same_reach_from_one_frame_to_the_next() {
    let ik = json!({ "chain": ["root", "arm", "hand"], "target": [40.0, 40.0] });
    let mut rt = runtime(json!({ "ik": [ik] }));
    rt.advance(1.0 / 60.0);
    let first = [at(&rt, "root"), at(&rt, "arm"), at(&rt, "hand")];
    for _ in 0..5 {
        rt.advance(1.0 / 60.0);
        let now = [at(&rt, "root"), at(&rt, "arm"), at(&rt, "hand")];
        assert!(first.iter().zip(&now).all(|(a, b)| close(*a, *b, 0.01)), "{first:?} {now:?}");
    }
}

#[test]
fn limits_of_a_longer_chain_may_bend_it_against_its_side() {
    let cases = [
        (json!([-150, -5]), Some("clockwise")),
        (json!([-150, -5]), None),
        (json!([5, 150]), Some("counterclockwise")),
    ];
    for (limits, bend) in cases {
        for target in [[0.0, 30.0], [0.0, -30.0], [10.0, 20.0], [10.0, -20.0]] {
            let mut more = json!({ "limits": { "arm": limits, "hand": limits } });
            if let Some(bend) = bend {
                more["bend"] = json!(bend);
            }
            let p = reached(target, more);
            assert!(close(p[3], target, 0.05), "{limits} {bend:?} {target:?}: {p:?}");
        }
    }
}
