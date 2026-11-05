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
