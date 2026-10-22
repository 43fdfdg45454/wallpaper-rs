use super::*;

#[test]
fn clouds_cover_part_of_the_layer() {
    let Some(out) = Fx::new("clouds", json!({ "size": 16 })).input(solid([0; 3])).shot() else { return };
    let values: Vec<u8> = out.0.pixels().map(|p| p.0[0]).collect();
    let (lo, hi) = (values.iter().min().copied().unwrap_or(0), values.iter().max().copied().unwrap_or(0));
    assert!(hi - lo > 60 && (10.0..200.0).contains(&out.mean()[0]), "{lo} {hi} {:?}", out.mean());
    assert!(out.mean()[2] >= out.mean()[0]);
    let clear = Fx::new("clouds", json!({ "size": 16, "coverage": 0 })).input(solid([0; 3]));
    let Some(clear) = clear.shot() else { return };
    assert!(clear.mean()[0] < 2.0, "{:?}", clear.mean());
    let low = Fx::new("clouds", json!({ "size": 16, "coverage": 1, "height": 0.2 })).input(solid([0; 3]));
    let Some(low) = low.shot() else { return };
    low.check(&[((10, 30), [0; 3]), ((50, 31), [0; 3])]);
    assert!(low.at(10, 0)[0] > 100);
}
