use super::*;

#[test]
fn dissolve_removes_pixels_with_a_glowing_edge() {
    let at = |progress: f32| {
        Fx::new("dissolve", json!({ "progress": progress, "size": 8 })).input(solid([0, 0, 255])).shot()
    };
    let (Some(whole), Some(half), Some(gone)) = (at(0.0), at(0.5), at(1.0)) else { return };
    assert!(whole.diff(&Shot(solid([0, 0, 255]))) < 0.5);
    gone.check(&[((10, 10), [0; 3]), ((40, 20), [0; 3])]);
    let mean = half.mean();
    assert!((40.0..215.0).contains(&mean[2]) && mean[0] > 2.0, "{mean:?}");
}
