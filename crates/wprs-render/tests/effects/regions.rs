use super::*;

fn corner(x0: u32, x1: u32) -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| {
        Rgba(if (x0..x1).contains(&x) && y < H / 2 { [255, 0, 0, 255] } else { [0, 0, 0, 255] })
    })
}

fn waves(mask: &str, strength: f32) -> Value {
    let params = json!({ "amplitude": 6, "frequency": 2, "speed": 0, "direction": 90 });
    json!({ "effect": "waves", "params": params, "mask": { "texture": mask, "channel": "r", "strength": strength } })
}

fn chain(strength: f32) -> Option<Shot> {
    let list = json!([
        waves("left", strength),
        waves("right", strength),
        waves("left", strength),
        waves("none", strength)
    ]);
    let fx =
        Fx::chain(list).input(ramp()).image("left", corner(4, 20), true).image("right", corner(40, 60), true);
    fx.image("none", corner(0, 0), true).shot()
}

#[test]
fn masked_effects_draw_only_where_their_mask_can_change_the_picture() {
    let (Some(narrow), Some(whole)) = (chain(1.0), chain(0.9999)) else { return };
    let most = narrow
        .0
        .pixels()
        .zip(whole.0.pixels())
        .map(|(a, b)| (0..3).map(|c| a.0[c].abs_diff(b.0[c])).max().unwrap_or(0))
        .max();
    assert!(most.unwrap_or(0) <= 2, "{most:?}");
    let Some(plain) = Fx::chain(json!([])).input(ramp()).shot() else { return };
    let moved = |x: u32, y: u32| narrow.at(x, y) != plain.at(x, y);
    assert!(moved(10, 8) && moved(50, 8), "{:?} {:?}", narrow.at(10, 8), plain.at(10, 8));
    assert!(!moved(30, 8) && !moved(10, 24) && !moved(62, 28));
}

#[test]
fn scenes_that_blend_in_srgb_draw_no_more_passes_than_linear_ones() {
    let passes = |space: &'static str| {
        let mut run =
            Fx::chain(json!([{ "effect": "tint", "params": { "color": "#ff0000" } }])).space(space).run()?;
        let shot = run.shot();
        Some((run.stats().passes, shot))
    };
    let (Some((srgb, a)), Some((linear, _))) = (passes("srgb"), passes("linear")) else { return };
    assert!(srgb <= linear && a.at(32, 16) != [0, 0, 0], "{srgb} passes in srgb, {linear} in linear");
}

fn after_waves(effect: Value, strength: f32) -> Option<Shot> {
    let mut far = effect;
    far["mask"] = json!({ "texture": "box", "channel": "r", "strength": strength });
    let list = json!([{ "effect": "waves", "params": { "amplitude": 0.5 } }, far]);
    Fx::chain(list).input(ramp()).image("box", corner(4, 20), true).shot()
}

#[test]
fn masked_effects_that_sample_far_from_their_pixel_keep_the_whole_picture_to_read() {
    let quad = json!({ "top_left": [-0.5, 0], "top_right": [0.5, 0], "bottom_right": [0.5, 1], "bottom_left": [-0.5, 1] });
    for effect in [json!({ "effect": "perspective", "params": quad }), json!({ "effect": "crt" })] {
        let (Some(narrow), Some(whole)) = (after_waves(effect.clone(), 1.0), after_waves(effect.clone(), 0.9999))
        else {
            return;
        };
        let most = narrow.0.pixels().zip(whole.0.pixels()).map(|(a, b)| (0..3).map(|c| a.0[c].abs_diff(b.0[c])).max());
        let most = most.flatten().max().unwrap_or(0);
        assert!(most <= 2, "{effect} differs by {most}: {:?} against {:?}", narrow.at(10, 8), whole.at(10, 8));
    }
}


#[test]
fn masked_effects_that_read_only_their_own_pixel_still_run_in_place() {
    let shafts = json!({ "effect": "light_shafts", "params": { "intensity": 2 } });
    let (Some(narrow), Some(whole)) = (after_waves(shafts.clone(), 1.0), after_waves(shafts, 0.9999)) else {
        return;
    };
    assert!(narrow.diff(&whole) < 0.5, "{}", narrow.diff(&whole));
}

#[test]
fn waves_on_two_masks_share_one_pass_and_keep_each_mask() {
    let feathered = |mask: &str| {
        let mut w = waves(mask, 1.0);
        w["mask"]["feather"] = json!(1);
        w
    };
    let run = |list: Value| {
        let fx = Fx::chain(list).input(ramp()).image("left", corner(4, 20), true);
        let mut run = fx.image("right", corner(40, 60), true).run()?;
        Some((run.shot(), run.stats().passes))
    };
    let fused = json!([waves("left", 1.0), waves("right", 1.0), waves("left", 1.0), waves("right", 1.0)]);
    let apart = json!([feathered("left"), feathered("right"), feathered("left"), feathered("right")]);
    let (Some((a, one)), Some((b, more)), Some((_, single))) =
        (run(fused), run(apart), run(json!([waves("left", 1.0)])))
    else {
        return;
    };
    assert!(one == single && one < more && a.diff(&b) < 1.0, "{one} {single} {more} {}", a.diff(&b));
    let Some(plain) = Fx::chain(json!([])).input(ramp()).shot() else { return };
    let moved = |x: u32, y: u32| a.at(x, y) != plain.at(x, y);
    assert!(moved(10, 8) && moved(50, 8) && !moved(30, 8) && !moved(10, 24));
}

const PUSH: &str = "@fragment
fn fs_push(in: VertexOut) -> @location(0) vec4<f32> {
    let lu = layer_uv(in.uv);
    let moved = from_layer(lu + vec2<f32>(param(0u).x * sin(frame.time + 1.0), 0.0));
    return mix(tap(in.uv), tap(moved), mask_at(moved));
}";

fn pushed(strength: f32) -> Option<Shot> {
    let push = json!({ "id": "push", "mask_mode": "strength", "cost": {},
        "params": [{ "id": "by", "type": "float", "default": 0.0 }], "passes": [{ "entry": "fs_push" }] });
    let mut far = json!({ "effect": "push", "params": { "by": 0.2 } });
    far["mask"] = json!({ "texture": "box", "channel": "r", "strength": strength });
    let list = json!([{ "effect": "waves", "params": { "amplitude": 0.5 } }, far]);
    Fx::chain(list).input(ramp()).image("box", corner(30, 40), true).own(push, PUSH).shot()
}

#[test]
fn masked_package_effects_that_read_a_bounded_way_off_run_in_place_over_their_reach() {
    let (Some(narrow), Some(whole)) = (pushed(1.0), pushed(0.9999)) else { return };
    let most = narrow.0.pixels().zip(whole.0.pixels()).map(|(a, b)| (0..3).map(|c| a.0[c].abs_diff(b.0[c])).max());
    assert!(most.flatten().max().unwrap_or(0) <= 2, "{:?} against {:?}", narrow.at(20, 8), whole.at(20, 8));
    let Some(plain) = Fx::chain(json!([{ "effect": "waves", "params": { "amplitude": 0.5 } }])).input(ramp()).shot()
    else {
        return;
    };
    assert!((18..30).any(|x| narrow.at(x, 8) != plain.at(x, 8)), "the push reaches left of its mask");
}
