use super::*;
use wprs_scene::Wake;

#[test]
fn trails_fade_at_any_frame_rate_and_settle_once_the_picture_stops() {
    let moving = json!({ "keys": [[0, [0, 0]], [0.5, [0.4, 0]]] });
    let chain = |p: f32| json!([{ "effect": "transform", "params": { "offset": moving } },
        { "effect": "trail", "params": { "persistence": p } }]);
    let run_at = |fps: f64| {
        let mut run = Fx::chain(chain(0.9)).input(spot()).run()?;
        for _ in 0..(0.3 * fps).round() as usize {
            run.runtime.advance(1.0 / fps);
            run.shot();
        }
        Some(run)
    };
    let (Some(mut fast), Some(mut slow)) = (run_at(60.0), run_at(30.0)) else { return };
    let (a, b) = (fast.shot(), slow.shot());
    let (m, n) = (a.mean()[0], b.mean()[0]);
    assert!((m - n).abs() < m * 0.25, "{m} {n}");
    let behind = (20..34).map(|x| a.at(x, 16)[0]).filter(|v| *v > 20 && *v < 240).count();
    assert!(behind >= 3, "no fading copies behind the dot");
    assert_eq!(fast.runtime.wake(), Wake::Frame);
    let mut frames = 0;
    while fast.runtime.wake() == Wake::Frame && frames < 600 {
        fast.runtime.advance(1.0 / 60.0);
        fast.shot();
        frames += 1;
    }
    assert!(fast.runtime.time() < 2.0, "{}", fast.runtime.time());
    let Some(still) = Fx::new("transform", json!({ "offset": [0.4, 0] })).input(spot()).shot() else { return };
    assert!(fast.shot().diff(&still) < 0.1, "{}", fast.shot().diff(&still));
    let Some(mut idle) = Fx::new("trail", json!({})).input(spot()).run() else { return };
    idle.shot();
    assert_eq!(idle.runtime.wake(), Wake::Idle);
    Shot::sheet(&[a, b, still]).looks_like("trail");
}
