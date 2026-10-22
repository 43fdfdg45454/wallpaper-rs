use super::*;

#[test]
fn film_grain_adds_noise_around_the_same_tone() {
    let fx = |fps: f32| Fx::new("film_grain", json!({ "amount": 0.3, "fps": fps })).input(solid([128; 3]));
    let Some(mut still) = fx(0.0).run() else { return };
    let first = still.shot();
    let values: Vec<u8> = first.0.pixels().map(|p| p.0[1]).collect();
    let spread = values.iter().max().unwrap_or(&0) - values.iter().min().unwrap_or(&0);
    assert!(spread > 30 && (first.mean()[1] - 128.0).abs() < 6.0, "{spread} {:?}", first.mean());
    still.runtime.seek(0.5);
    assert_eq!(still.shot().diff(&first), 0.0);
    let Some(mut moving) = fx(24.0).run() else { return };
    let first = moving.shot();
    moving.runtime.seek(0.1);
    assert!(moving.shot().diff(&first) > 2.0);
}
