use super::*;

fn eye() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| {
        Rgba(if x.abs_diff(32) < 3 && y.abs_diff(16) < 3 { [255; 4] } else { [0, 0, 0, 255] })
    })
}

#[test]
fn iris_follows_the_pointer_and_wanders_without_it() {
    let Some(mut run) = Fx::new("iris", json!({ "follow": 1 })).input(eye()).run() else { return };
    run.pointer([1.0, 0.5]);
    run.shot().check(&[((44, 16), [255; 3]), ((32, 16), [0; 3])]);
    let Some(still) = Fx::new("iris", json!({ "saccades": false })).input(eye()).shot() else { return };
    assert!(still.diff(&Shot(eye())) < 0.5);
    let Some(mut run) = Fx::new("iris", json!({})).input(eye()).run() else { return };
    let shots: Vec<Shot> = (0..60)
        .map(|_| {
            run.runtime.advance(0.1);
            run.shot()
        })
        .collect();
    let jumps = shots.windows(2).filter(|w| w[0].diff(&w[1]) > 0.0).count();
    assert!((3..30).contains(&jumps), "{jumps}");
    assert!(shots.iter().all(|s| s.mean()[0] > 2.0), "the iris left the eye");
}
