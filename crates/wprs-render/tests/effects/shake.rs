use super::*;

#[test]
fn shake_is_still_by_default_and_moves_when_enabled() {
    let Some(mut off) = Fx::new("shake", json!({})).input(ramp()).run() else { return };
    off.runtime.advance(0.3);
    assert!(off.shot().diff(&Shot(ramp())) < 0.5);
    let Some(mut on) = Fx::new("shake", json!({ "strength": 6 })).input(ramp()).run() else { return };
    let mut shots = Vec::new();
    for _ in 0..4 {
        on.runtime.advance(0.37);
        shots.push(on.shot());
    }
    assert!(shots.windows(2).any(|w| w[0].diff(&w[1]) > 1.0));
    assert!(shots.iter().all(|s| s.at(32, 16)[0].abs_diff(Shot(ramp()).at(32, 16)[0]) <= 26));
}
