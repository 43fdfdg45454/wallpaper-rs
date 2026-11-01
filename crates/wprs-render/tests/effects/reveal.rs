use super::*;

#[test]
fn reveal_shows_another_picture_through_a_shape_that_follows_the_pointer() {
    let fx = |params: Value| Fx::new("reveal", params).input(solid(RED)).image("hidden", solid(BLUE), false);
    let Some(round) = fx(json!({ "texture": "hidden", "radius": 0.2, "softness": 0 })).shot() else { return };
    round.check(&[((32, 16), BLUE), ((2, 16), RED), ((32, 1), RED)]);
    let Some(inverted) = fx(json!({ "texture": "hidden", "radius": 0.2, "softness": 0, "invert": true })).shot() else {
        return;
    };
    inverted.check(&[((32, 16), RED), ((2, 16), BLUE)]);
    let Some(oval) = fx(json!({ "texture": "hidden", "radius": 0.3, "softness": 0, "shape": 1 })).shot() else { return };
    oval.check(&[((50, 16), BLUE), ((32, 4), RED)]);
    let Some(mut run) = fx(json!({ "texture": "hidden", "radius": 0.2, "softness": 0, "follow": true })).run() else {
        return;
    };
    run.pointer([0.85, 0.5]);
    let followed = run.shot();
    followed.check(&[((54, 16), BLUE), ((32, 16), RED)]);
    Shot::sheet(&[round, inverted, oval, followed]).looks_like("reveal");
}

#[test]
fn gradient_blends_cross_through_a_map() {
    let fx = |params: Value| {
        let fx = Fx::new("blend_gradient", params).input(solid(RED)).image("other", solid(BLUE), false);
        fx.image("map", RgbaImage::from_fn(W, H, |_, y| Rgba([(y * 8) as u8, 0, 0, 255])), true).shot()
    };
    let Some(none) = fx(json!({ "texture": "other", "progress": 0 })) else { return };
    none.check(&[((10, 10), RED)]);
    let Some(all) = fx(json!({ "texture": "other", "progress": 1 })) else { return };
    all.check(&[((10, 10), BLUE), ((60, 30), BLUE)]);
    let Some(across) = fx(json!({ "texture": "other", "progress": 0.5, "softness": 0.5 })) else { return };
    let Some(plain) = fx(json!({ "texture": "other", "progress": 0.5, "softness": 0 })) else { return };
    let Some(mapped) = fx(json!({ "texture": "other", "progress": 0.5, "softness": 0, "map": "map" })) else { return };
    mapped.check(&[((32, 4), BLUE), ((32, 28), RED)]);
    plain.check(&[((10, 16), BLUE), ((54, 16), RED)]);
    Shot::sheet(&[none, across, mapped, plain]).looks_like("blend_gradient");
}

#[test]
fn lens_flares_place_ghosts_along_the_axis_and_follow_the_pointer() {
    let Some(flare) = Fx::new("lens_flare", json!({ "source": [0.2, 0.3] })).input(solid([0; 3])).shot() else { return };
    assert!(flare.at(13, 10)[0] > 150 && flare.mean()[0] > 4.0, "{:?} {:?}", flare.at(13, 10), flare.mean());
    let Some(none) = Fx::new("lens_flare", json!({ "ghosts": 0, "halo": 0 })).input(solid([0; 3])).shot() else { return };
    let Some(mut run) = Fx::new("lens_flare", json!({ "follow": true })).input(solid([0; 3])).run() else { return };
    run.pointer([0.8, 0.8]);
    let followed = run.shot();
    assert!(followed.at(51, 25)[0] > 150, "{:?}", followed.at(51, 25));
    differ(&[&flare, &none, &followed], 0.5);
    Shot::sheet(&[flare, none, followed]).looks_like("lens_flare");
}
