use super::*;

#[test]
fn color_adjust_changes_saturation_and_exposure() {
    let Some(gray) = Fx::new("color_adjust", json!({ "saturation": 0 })).input(solid(RED)).shot() else { return };
    gray.check(&[((32, 16), [127, 127, 127])]);
    let brighter = Fx::new("color_adjust", json!({ "exposure": 1 })).input(solid([128; 3])).adjust();
    let Some(bright) = brighter.shot() else { return };
    bright.check(&[((32, 16), [175, 175, 175])]);
}
