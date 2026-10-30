use super::*;

const PACK: &str = r#"
override boost: i32 = 1;
override flip: bool = false;

@fragment
fn fs_pack_fill(in: VertexOut) -> @location(0) vec4<f32> {
    return tap(in.uv) * f32(boost) * 0.5;
}

@fragment
fn fs_pack_add(in: VertexOut) -> @location(0) vec4<f32> {
    return sample(input_tex, in.uv) * param(0u).x;
}

@vertex
fn vs_pack(@builtin(vertex_index) i: u32) -> VertexOut {
    let uv = grid_uv(i);
    return VertexOut(grid_position(uv + vec2<f32>(0.0, 0.25 * sin(uv.x * PI))), uv);
}

@fragment
fn fs_pack_show(in: VertexOut) -> @location(0) vec4<f32> {
    let k = min(u32(in.uv.x * 8.0), 7u);
    var c = vec4<f32>(0.0);
    switch k {
        case 0u: { c = sample(aux0, in.uv); }
        case 1u: { c = sample(aux1, in.uv); }
        case 2u: { c = sample(aux2, in.uv); }
        case 3u: { c = sample(aux3, in.uv); }
        case 4u: { c = sample(aux4, in.uv); }
        case 5u: { c = sample(aux5, in.uv); }
        case 6u: { c = sample(aux6, in.uv); }
        default: { c = sample(aux7, in.uv); }
    }
    let size = tex_size(k).x / 8.0;
    let rg = select(vec2<f32>(c.r, size), vec2<f32>(size, c.r), flip);
    return vec4<f32>(rg, tap(in.uv).b, 1.0);
}
"#;

fn pack() -> Value {
    let textures = (0..8).map(|k| json!({ "id": format!("t{k}"), "type": "texture", "default": "" }));
    let mut params = vec![json!({ "id": "amount", "type": "float", "default": 0.25, "min": 0, "max": 1 })];
    params.extend(textures);
    let inputs: Vec<String> = std::iter::once("pass:4".into()).chain((0..8).map(|k| format!("asset:t{k}"))).collect();
    json!({ "id": "pack", "params": params, "cost": { "taps": 2, "passes": 6, "scale": 1 },
        "targets": { "a": { "scale": 0.5, "format": "rgba16f" }, "b": { "scale": 0.5, "format": "rgba16f" },
                     "acc": { "scale": 1 } },
        "options": [{ "id": "boost", "values": [1, 2], "default": 1 },
                    { "id": "flip", "values": [false, true], "default": false, "when_bound": "t7" }],
        "passes": [
            { "entry": "fs_pack_fill", "target": "a" },
            { "swap": ["a", "b"] },
            { "entry": "fs_pack_add", "target": "acc", "blend": "add", "inputs": ["target:b"] },
            { "entry": "fs_pack_add", "target": "acc", "blend": "add", "inputs": ["target:b"] },
            { "copy": "target:acc" },
            { "entry": "fs_pack_show", "vertex": "vs_pack", "grid": [8, 8], "inputs": inputs }
        ] })
}

fn enc(v: f32) -> u8 {
    let c = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round() as u8
}

fn dec(v: u8) -> f32 {
    let c = f32::from(v) / 255.0;
    if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn fx(params: Value) -> Fx {
    with(params, pack())
}

fn with(params: Value, pack: Value) -> Fx {
    let fx = Fx::new("pack", params).own(pack, PACK);
    (0..8u32).fold(fx, |fx, k| {
        let red = (32 * (k + 1) - 1) as u8;
        fx.image(&format!("t{k}"), RgbaImage::from_pixel(k + 1, 1, Rgba([red, 0, 0, 255])), false)
    })
}

#[test]
fn package_effects_run_passes_targets_variants_grids_and_eight_textures() {
    let textures = (0..8).map(|k| (format!("t{k}"), json!(format!("t{k}"))));
    let mut params: Map<String, Value> = textures.collect();
    params.insert("boost".into(), json!(2));
    let Some(mut run) = fx(Value::Object(params.clone())).run() else { return };
    let shot = run.shot();
    assert!(run.view.left_out().is_empty(), "{:?}", run.view.left_out());
    shot.looks_like("package");
    for k in 0..8u32 {
        let x = 8 * k + 4;
        let ramp = (x * 255 / (W - 1)) as u8;
        let [r, g, b] = shot.at(x, 28);
        let blue = enc(0.5 * dec(ramp));
        let (red, green) = (enc((k + 1) as f32 / 8.0), (32 * (k + 1) - 1) as u8);
        assert!(r.abs_diff(red) <= 3 && g.abs_diff(green) <= 3, "stripe {k} is {r}, {g}, expected {red}, {green}");
        assert!(k < 3 || b.abs_diff(blue) <= 3, "stripe {k} has blue {b}, expected {blue}");
    }
    shot.check(&[((32, 2), [0, 0, 0])]);
    assert!(run.view.left_out().is_empty(), "{:?}", run.view.left_out());
    params.insert("boost".into(), json!(1));
    params.insert("flip".into(), json!(false));
    let Some(mut plain) = fx(Value::Object(params)).run() else { return };
    let ramp = (36 * 255 / (W - 1)) as u8;
    plain.shot().check(&[((36, 28), [159, enc(5.0 / 8.0), enc(0.25 * dec(ramp))])]);
    assert_eq!(plain.stats().compiled, 0, "every variant was built with the scene");
}

#[test]
fn a_stopped_package_effect_is_left_out_with_a_message() {
    let mut other = pack();
    other["cost"]["taps"] = json!(3);
    let Some(mut run) = with(json!({}), other).run() else { return };
    wprs_render::stop_effects([run.runtime.scene.loaded["pack"].key]);
    let shot = run.shot();
    shot.check(&[((32, 16), [128; 3])]);
    let said = run.view.left_out();
    assert!(said.len() == 1 && said[0].contains("the effect pack was left out"), "{said:?}");
    assert!(wprs_render::retry_effects() >= 1);
    run.view.retry();
    let again = run.shot();
    assert!(run.view.left_out().is_empty() && again.at(32, 16) != [128; 3], "{:?}", again.at(32, 16));
}

#[test]
fn unpainted_masks_and_generic_textures_read_their_defaults() {
    let wgsl = "@fragment fn fs_paint(in: VertexOut) -> @location(0) vec4<f32> {
    return select(sample(aux1, in.uv), sample(aux0, in.uv), in.uv.x < 0.5);
}";
    let json = json!({ "id": "paint", "cost": {}, "params": [
        { "id": "cover", "type": "mask", "default": "", "paint": "#ff0000" },
        { "id": "flat", "type": "texture", "default": "normal_flat" }],
        "passes": [{ "entry": "fs_paint", "inputs": ["previous", "asset:cover", "asset:flat"] }] });
    let Some(shot) = Fx::new("paint", json!({})).own(json, wgsl).shot() else { return };
    let mid = enc(128.0 / 255.0);
    shot.check(&[((8, 16), [255, 0, 0]), ((56, 16), [mid, mid, 255])]);
}

#[test]
fn package_effects_read_sixteen_and_thirty_two_bands_per_channel() {
    let wgsl = "@fragment fn fs_bands(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(band_left_of(1u, 16u), band_right_of(2u, 32u), band_left(0u) + band_of(0u, 16u), 1.0);
}";
    let json = json!({ "id": "bands", "cost": {}, "passes": [{ "entry": "fs_bands" }] });
    let Some(mut run) = Fx::new("bands", json!({})).own(json, wgsl).run() else { return };
    let mut audio = wprs_scene::AudioFrame { silent: false, ..Default::default() };
    audio.left[4..8].fill(0.5);
    audio.right[4..6].fill(1.0);
    run.runtime.set_audio(audio);
    run.shot().check(&[((32, 16), [enc(0.5), 255, 0])]);
}

#[test]
fn an_option_waiting_for_a_texture_turns_on_when_a_layer_feeds_it() {
    let wgsl = "override lit: bool = false;
@fragment fn fs_lit(in: VertexOut) -> @location(0) vec4<f32> {
    return select(vec4<f32>(0.0, 0.0, 1.0, 1.0), vec4<f32>(1.0, 0.0, 0.0, 1.0), lit);
}";
    let json = json!({ "id": "lit", "cost": {}, "params": [{ "id": "shape", "type": "texture", "default": "" }],
        "options": [{ "id": "lit", "values": [false, true], "default": false, "when_bound": "shape" }],
        "passes": [{ "entry": "fs_lit", "inputs": ["previous", "asset:shape"] }] });
    let dot = json!({ "id": "dot", "type": "solid", "color": "#ffffff", "size": [8, 8], "visible": false });
    let fed = Fx::new("lit", json!({ "shape": { "layer": "dot" } })).own(json.clone(), wgsl).layer(dot);
    let Some(mut run) = fed.run() else { return };
    run.shot().check(&[((32, 16), [255, 0, 0])]);
    assert_eq!(run.stats().compiled, 0, "the variant was built with the scene");
    let Some(plain) = Fx::new("lit", json!({})).own(json, wgsl).shot() else { return };
    plain.check(&[((32, 16), [0, 0, 255])]);
}

#[test]
fn compose_effects_read_the_scene_below_and_others_may_read_the_backdrop() {
    let half = RgbaImage::from_fn(W, H, |x, _| Rgba(if x < W / 2 { [255, 0, 0, 255] } else { [0; 4] }));
    let swap = "@fragment fn fs_turn(in: VertexOut) -> @location(0) vec4<f32> { return vec4<f32>(tap(in.uv).bgr, 1.0); }";
    let turn = json!({ "id": "turn", "compose": true, "cost": {}, "passes": [{ "entry": "fs_turn" }] });
    let Some(composed) = Fx::new("turn", json!({})).own(turn, swap).input(half.clone()).background("#0000ff").shot()
    else {
        return;
    };
    composed.check(&[((8, 16), [0, 0, 255]), ((56, 16), [255, 0, 0])]);
    let below = "@fragment fn fs_below(in: VertexOut) -> @location(0) vec4<f32> { return vec4<f32>(sample(aux0, in.uv).bgr, 1.0); }";
    let back = json!({ "id": "below", "cost": {}, "passes": [{ "entry": "fs_below", "inputs": ["previous", "backdrop"] }] });
    let Some(shot) = Fx::new("below", json!({})).own(back, below).input(half).background("#0000ff").shot() else {
        return;
    };
    shot.check(&[((8, 16), [255, 0, 0]), ((56, 16), [255, 0, 0])]);
}
