use super::*;

#[test]
fn vignette_darkens_corners_and_keeps_the_center() {
    let Some(out) = Fx::new("vignette", json!({ "intensity": 0.8 })).input(solid([255; 3])).shot() else {
        return;
    };
    out.check(&[((32, 16), [255; 3])]);
    let (corner, side, near) = (out.at(0, 0)[0], out.at(0, 16)[0], out.at(24, 16)[0]);
    assert!(corner < side && side < near && corner < 150, "{corner} {side} {near}");
    let Some(soft) = Fx::new("vignette", json!({})).input(solid([255; 3])).shot() else { return };
    assert!(soft.at(0, 0)[0] > corner && soft.at(0, 0)[0] < 255, "{:?}", soft.at(0, 0));
}
