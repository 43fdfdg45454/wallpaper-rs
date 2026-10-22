use super::*;
use std::collections::BTreeSet;

fn levels(shot: &Shot) -> Vec<u8> {
    shot.0.pixels().map(|p| p.0[0]).collect::<BTreeSet<u8>>().into_iter().collect()
}

#[test]
fn posterize_yields_its_levels() {
    let Some(flat) = Fx::new("posterize", json!({ "levels": 4, "dither": 0 })).shot() else { return };
    let found = levels(&flat);
    assert!(
        found.len() == 4 && found.iter().zip([0, 85, 170, 255]).all(|(a, b)| a.abs_diff(b) <= 2),
        "{found:?}"
    );
    let gray =
        |dither: f32| Fx::new("posterize", json!({ "levels": 4, "dither": dither })).input(solid([128; 3]));
    let Some(banded) = gray(0.0).shot() else { return };
    let Some(dithered) = gray(1.0).shot() else { return };
    assert_eq!(levels(&banded).len(), 1);
    assert_eq!(levels(&dithered).len(), 2);
    assert!((dithered.mean()[0] - 128.0).abs() < (banded.mean()[0] - 128.0).abs(), "{:?}", dithered.mean());
}
