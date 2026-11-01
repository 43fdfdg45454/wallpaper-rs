use super::*;

#[test]
fn blur_blends_its_result_into_the_picture() {
    let blur = |params: Value| Fx::new("blur", params).input(halves(RED, BLUE)).shot();
    let Some(none) = blur(json!({ "radius": 12, "amount": 0 })) else { return };
    none.check(&[((31, 16), RED), ((32, 16), BLUE)]);
    let Some(lighter) = blur(json!({ "radius": 12, "blend": "lighten" })) else { return };
    let [r, _, b] = lighter.at(34, 16);
    assert!(r > 40 && b > 250, "{:?}", lighter.at(34, 16));
    let Some(half) = blur(json!({ "radius": 12, "amount": 0.5 })) else { return };
    let Some(full) = blur(json!({ "radius": 12 })) else { return };
    differ(&[&none, &half, &full, &lighter], 0.5);
    Shot::sheet(&[none, half, full, lighter]).looks_like("blur_blends");
}

#[test]
fn chromatic_aberration_splits_colors_in_four_ways() {
    let split = |mode: u32| Fx::new("chromatic_aberration", json!({ "amount": 4, "mode": mode })).input(spot()).shot();
    let Some(sideways) = split(1) else { return };
    sideways.check(&[((27, 16), [255, 0, 0]), ((36, 16), [0, 0, 255])]);
    let (Some(out), Some(radial), Some(barrel)) = (split(0), split(2), split(3)) else { return };
    differ(&[&out, &sideways, &radial, &barrel], 0.05);
    Shot::sheet(&[out, sideways, radial, barrel]).looks_like("chromatic_modes");
}

#[test]
fn film_grain_turns_grey_leans_to_dark_or_bright_parts_and_blends() {
    let grain = |params: Value, input: RgbaImage| Fx::new("film_grain", params).input(input).shot();
    let Some(grey) = grain(json!({ "amount": 0, "mono": true }), solid(RED)) else { return };
    let [r, g, b] = grey.at(10, 10);
    assert!(r.abs_diff(g) < 12 && g.abs_diff(b) < 12 && r > 40, "{:?}", grey.at(10, 10));
    let spread = |s: &Shot, x0: u32| {
        let v: Vec<u8> = (x0..x0 + 12).flat_map(|x| (0..H).map(move |y| (x, y))).map(|(x, y)| s.at(x, y)[1]).collect();
        v.iter().max().unwrap_or(&0) - v.iter().min().unwrap_or(&0)
    };
    let (Some(dark), Some(bright)) = (
        grain(json!({ "amount": 0.4, "bias": -1 }), ramp()),
        grain(json!({ "amount": 0.4, "bias": 1 }), ramp()),
    ) else {
        return;
    };
    assert!(spread(&dark, 10) > spread(&bright, 10) && spread(&bright, 42) > spread(&dark, 42));
    let Some(multiplied) = grain(json!({ "amount": 0.4, "blend": "multiply" }), ramp()) else { return };
    assert!(multiplied.mean()[1] + 2.0 < dark.mean()[1], "{:?} {:?}", multiplied.mean(), dark.mean());
    Shot::sheet(&[grey, dark, bright, multiplied]).looks_like("grain_styles");
}

#[test]
fn shine_blends_its_band_and_reads_a_gradient() {
    let band = json!({ "angle": 0, "width": 0.25, "softness": 0, "intensity": 1, "speed": 0, "phase": 0.5,
        "color": "#ff0000", "blend": "normal" });
    let Some(normal) = Fx::new("shine", band.clone()).input(solid([128; 3])).shot() else { return };
    normal.check(&[((32, 16), [255, 0, 0]), ((4, 16), [128; 3])]);
    let mut colored = band;
    colored["gradient"] = json!("ramp");
    let fx = Fx::new("shine", colored).input(solid([128; 3])).image("ramp", halves([0, 255, 0], [0, 0, 255]), false);
    let Some(ramped) = fx.shot() else { return };
    ramped.check(&[((29, 16), [0, 255, 0]), ((35, 16), [0, 0, 255])]);
    Shot::sheet(&[normal, ramped]).looks_like("shine_styles");
}

#[test]
fn light_rays_also_run_in_one_direction_or_as_a_star() {
    let rays = |params: Value| {
        let mut p = json!({ "threshold": 0.5, "exposure": 2, "density": 1, "decay": 1 });
        p.as_object_mut().unwrap().extend(params.as_object().unwrap().clone());
        Fx::new("god_rays", p).input(spot()).shot()
    };
    let Some(along) = rays(json!({ "mode": 1, "angle": 0 })) else { return };
    let lit = |s: &Shot, x, y| s.at(x, y)[0];
    assert!(lit(&along, 44, 16) > lit(&along, 20, 16) + 20, "{} {}", lit(&along, 44, 16), lit(&along, 20, 16));
    let Some(star) = rays(json!({ "mode": 2, "edges": 4, "angle": 0 })) else { return };
    assert!(lit(&star, 32, 8) > 20 && lit(&star, 40, 16) > 20 && lit(&star, 40, 8) < lit(&star, 32, 8));
    let Some(radial) = rays(json!({ "source": [0.1, 0.5] })) else { return };
    differ(&[&along, &star, &radial], 0.2);
    Shot::sheet(&[along, star, radial]).looks_like("ray_modes");
}
