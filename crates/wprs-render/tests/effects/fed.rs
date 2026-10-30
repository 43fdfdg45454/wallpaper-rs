use super::*;

#[test]
fn effects_take_masks_from_layers_and_shapes() {
    let half = json!({ "id": "half", "type": "solid", "color": "#ffffff", "size": [32, 32], "visible": false,
        "transform": { "position": [48, 16] } });
    let gray = |mask: Value| json!([{ "effect": "color_adjust", "params": { "saturation": 0 }, "mask": mask }]);
    let Some(layered) = Fx::chain(gray(json!({ "layer": "half" }))).input(solid(RED)).layer(half.clone()).shot()
    else {
        return;
    };
    let [r, g, _] = layered.at(56, 16);
    assert!(layered.at(8, 16) == RED && r.abs_diff(g) < 8, "{:?}", layered.at(56, 16));
    let Some(inverted) =
        Fx::chain(gray(json!({ "layer": "half", "invert": true }))).input(solid(RED)).layer(half).shot()
    else {
        return;
    };
    assert!(inverted.at(56, 16) == RED && inverted.at(8, 16) != RED, "{:?}", inverted.at(8, 16));
    let ring = json!({ "shape": { "ellipse": { "size": [40, 24] } }, "feather": 3, "strength": 0.5 });
    let Some(soft) = Fx::chain(gray(ring)).input(solid(RED)).shot() else { return };
    let [r, g, _] = soft.at(1, 1);
    let [x, y, _] = soft.at(32, 16);
    assert!(r > g + 40 && g > 20 && x.abs_diff(y) < 8, "{:?} {:?}", soft.at(1, 1), soft.at(32, 16));
    Shot::sheet(&[layered, inverted, soft]).looks_like("fed");
}

#[test]
fn built_in_effects_read_the_backdrop() {
    let json = json!({ "id": "under_probe", "passes": [{ "entry": "fs_under_probe", "inputs": ["previous", "backdrop"] }] });
    probe(json, "@fragment\nfn fs_under_probe(in: VertexOut) -> @location(0) vec4<f32> {\n    return sample(aux0, in.uv);\n}\n");
    let Some(out) = Fx::new("under_probe", json!({})).input(solid(RED)).background("#00ff00").shot() else { return };
    out.check(&[((32, 16), [0, 255, 0]), ((4, 4), [0, 255, 0])]);
}

fn moving(id: &str) -> Value {
    json!({ "id": id, "type": "solid", "color": "#ffffff", "size": [16, 16], "visible": false,
        "transform": { "position": { "keys": [[0, [8, 8]], [1, [56, 24]]] } } })
}

fn played(fx: impl Fn() -> Fx) -> Option<(Shot, Shot, u32)> {
    let mut run = fx().run()?;
    run.shot();
    let mut damage = 0;
    for _ in 0..30 {
        run.runtime.advance(1.0 / 30.0);
        run.shot();
        damage = damage.max(run.stats().damage);
    }
    let last = run.shot();
    let mut fresh = fx().run()?;
    fresh.runtime.advance(1.0);
    Some((last, fresh.shot(), damage))
}

#[test]
fn effects_follow_hidden_moving_layers_they_read() {
    let texture = json!([{ "effect": "blend_texture", "params": { "texture": { "layer": "fed" }, "blend": "normal" } }]);
    let mask = json!([{ "effect": "tint", "params": { "color": "#ff0000" }, "mask": { "layer": "fed" } }]);
    let mid = json!({ "id": "mid", "type": "solid", "color": "#000000", "size": [W, H], "visible": false,
        "effects": [{ "effect": "blend_texture", "params": { "texture": { "layer": "fed" }, "blend": "normal" } }] });
    let chained = json!([{ "effect": "blend_texture", "params": { "texture": { "layer": "mid" }, "blend": "normal" } }]);
    let cases = [(texture, None), (mask, None), (chained, Some(mid))];
    for (effects, extra) in cases {
        let fx = || {
            let fx = Fx::chain(effects.clone()).input(solid(BLUE)).layer(moving("fed"));
            extra.clone().into_iter().fold(fx, Fx::layer)
        };
        let Some((last, fresh, damage)) = played(fx) else { return };
        assert!(damage > 0 && last.diff(&fresh) < 0.5, "{effects}: {} apart, damage {damage}", last.diff(&fresh));
    }
}

#[test]
fn a_layer_read_by_many_effects_is_drawn_once_a_frame() {
    let read = |next: &str| {
        let params = json!({ "texture": { "layer": next }, "blend": "screen", "amount": 0.5 });
        json!([{ "effect": "blend_texture", "params": params }, { "effect": "blend_texture", "params": params }])
    };
    let layer = |k: usize| {
        let mut l = moving(&format!("l{k}"));
        l["effects"] = if k < 9 { read(&format!("l{}", k + 1)) } else { json!([]) };
        l
    };
    let fx = (0..10).map(layer).fold(Fx::chain(read("l0")), Fx::layer);
    let Some(mut run) = fx.run() else { return };
    run.shot();
    run.runtime.advance(0.5);
    run.shot();
    let passes = run.stats().passes;
    assert!(passes > 10 && passes < 150, "{:?}", run.stats());
}

#[test]
fn masks_that_read_groups_draw_each_group_once_a_frame() {
    let passes = |depth: usize| {
        let group = |k: usize| {
            let masked = |n: usize| {
                let mut l = moving(&format!("s{k}_{n}"));
                l["visible"] = json!(true);
                if k + 1 < depth {
                    l["mask"] = json!({ "layer": format!("g{}", k + 1) });
                }
                l
            };
            json!({ "id": format!("g{k}"), "type": "group", "visible": false, "layers": [masked(0), masked(1)] })
        };
        let read = json!([{ "effect": "blend_texture", "params": { "texture": { "layer": "g0" }, "blend": "normal" } }]);
        let mut run = (0..depth).map(group).fold(Fx::chain(read), Fx::layer).run()?;
        run.shot();
        run.runtime.advance(0.5);
        run.shot();
        Some(run.stats().passes)
    };
    let (Some(short), Some(long)) = (passes(4), passes(12)) else { return };
    assert!(long < 3 * short, "{short} passes at depth 4, {long} at depth 12");
}

#[test]
fn groups_with_masked_children_that_feed_each_other_through_effects_draw_once_a_frame() {
    let passes = |depth: usize| {
        let read = |k: usize| {
            let params = json!({ "texture": { "layer": format!("g{k}") }, "blend": "normal" });
            json!([{ "effect": "blend_texture", "params": params }])
        };
        let group = |k: usize| {
            let next = k + 1 < depth;
            let child = |n: usize| {
                let mut l = moving(&format!("s{k}_{n}"));
                l["visible"] = json!(true);
                l["mask"] = match next {
                    true => json!({ "layer": format!("g{}", k + 1) }),
                    false => json!({ "shape": { "ellipse": { "size": [8, 8] } } }),
                };
                l["effects"] = if next { read(k + 1) } else { json!([]) };
                l
            };
            let mut g = json!({ "id": format!("g{k}"), "type": "group", "visible": false, "layers": [child(0), child(1)] });
            g["effects"] = if next { read(k + 1) } else { json!([]) };
            g
        };
        let mut run = (0..depth).map(group).fold(Fx::chain(read(0)), Fx::layer).run()?;
        run.shot();
        run.runtime.advance(0.5);
        run.shot();
        Some(run.stats().passes)
    };
    let (Some(four), Some(eight), Some(twelve)) = (passes(4), passes(8), passes(12)) else { return };
    let grown = twelve - eight;
    assert!(grown <= eight - four + 2 && twelve < 4 * four, "{four}, {eight} and {twelve} passes at depth 4, 8 and 12");
}
