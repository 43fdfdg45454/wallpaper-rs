use super::*;
use wprs_scene::AudioFrame;

fn cube(map: impl Fn([u8; 3]) -> [u8; 3]) -> String {
    let rows = (0..8u8).map(|i| map([i & 1, (i >> 1) & 1, i >> 2])).map(|[r, g, b]| format!("{r} {g} {b}\n"));
    format!("LUT_3D_SIZE 2\n{}", rows.collect::<String>())
}

#[test]
fn state_persists_between_frames_and_restarts_with_the_view() {
    probe(
        json!({ "id": "probe_state", "uses": ["time"], "state": { "scale": 0.5 }, "passes": [
            { "entry": "fs_probe_step", "inputs": ["state"], "output": "state" },
            { "entry": "fs_probe_show", "inputs": ["state"] }] }),
        "@fragment fn fs_probe_step(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) + 0.25; }
         @fragment fn fs_probe_show(in: VertexOut) -> @location(0) vec4<f32> { return vec4<f32>(tap(in.uv).rrr, 1.0); }",
    );
    let Some(mut run) = Fx::new("probe_state", json!({})).run() else { return };
    for level in [137, 188, 225] {
        run.shot().check(&[((32, 16), [level; 3])]);
        run.runtime.advance(1.0 / 60.0);
    }
    assert_eq!(run.stats().created, 0);
    run.view.trim();
    run.shot().check(&[((32, 16), [137; 3])]);
}

#[test]
fn luts_and_texture_assets_reach_the_shader() {
    probe(
        json!({ "id": "probe_assets", "params": [{ "id": "grade", "type": "lut", "default": "" },
            { "id": "depth", "type": "texture", "default": "" }],
            "passes": [{ "entry": "fs_probe_assets", "inputs": ["previous", "asset:depth"] }] }),
        "@fragment fn fs_probe_assets(in: VertexOut) -> @location(0) vec4<f32> {
             return vec4<f32>(lut(tap(in.uv).rgb).rg, asset_at(aux0, in.uv).r, 1.0);
         }",
    );
    let fx = |depth: &str| {
        Fx::new("probe_assets", json!({ "grade": "invert", "depth": depth }))
            .input(solid(RED))
            .lut("invert", &cube(|c| c.map(|v| 1 - v)))
            .image("data", halves([128; 3], [255; 3]), true)
            .image("photo", solid([128; 3]), false)
    };
    let Some(data) = fx("data").shot() else { return };
    data.check(&[((10, 16), [0, 255, 188]), ((54, 16), [0, 255, 255])]);
    let Some(photo) = fx("photo").shot() else { return };
    photo.check(&[((10, 16), [0, 255, 128])]);
    let Some(none) = Fx::new("probe_assets", json!({})).shot() else { return };
    none.check(&[((10, 16), [0, 0, 0])]);
}

#[test]
fn cursor_and_audio_reach_effects() {
    probe(
        json!({ "id": "probe_inputs", "uses": ["cursor", "audio"], "passes": [{ "entry": "fs_probe_inputs" }] }),
        "@fragment fn fs_probe_inputs(in: VertexOut) -> @location(0) vec4<f32> {
             let near = select(0.0, 1.0, distance(screen_uv(in.uv), frame.cursor) < 0.05);
             return vec4<f32>(near, frame.beat, max(frame.bass, band(5u)), 1.0);
         }",
    );
    let Some(mut run) = Fx::new("probe_inputs", json!({})).run() else { return };
    run.shot().check(&[((16, 16), [0, 0, 0]), ((48, 8), [0, 0, 0])]);
    run.pointer([0.75, 0.25]);
    let mut audio = AudioFrame { beat: true, silent: false, ..AudioFrame::default() };
    audio.average[5] = 1.0;
    run.runtime.set_audio(audio);
    run.shot().check(&[((48, 8), [255, 255, 255]), ((16, 16), [0, 255, 255])]);
}

#[test]
fn fused_chains_match_separate_passes() {
    let waves = json!({ "effect": "waves", "params": { "amplitude": 10, "frequency": 1, "speed": 0, "direction": 90 } });
    let gray = json!({ "effect": "color_adjust", "params": { "saturation": 0.5 } });
    let masked = json!({ "effect": "color_adjust", "params": { "saturation": 0.5 }, "mask": "white" });
    let run = |effects: Value| {
        let fx = Fx::chain(effects).input(halves(RED, BLUE)).image("white", solid([255; 3]), false);
        let mut run = fx.run()?;
        Some((run.shot(), run.stats().passes))
    };
    let Some((fused, one)) = run(json!([waves, gray])) else { return };
    let Some((apart, more)) = run(json!([waves, masked])) else { return };
    assert!(one < more && fused.diff(&apart) < 1.0, "{one} {more} {}", fused.diff(&apart));
    fused.check(&[((40, 16), apart.at(40, 16)), ((50, 16), apart.at(50, 16))]);
}

#[test]
fn masks_mix_and_noise_helpers_vary() {
    probe(
        json!({ "id": "probe_noise", "passes": [{ "entry": "fs_probe_noise" }] }),
        "@fragment fn fs_probe_noise(in: VertexOut) -> @location(0) vec4<f32> {
             let p = in.uv * 8.0;
             return vec4<f32>(value_noise(p), fbm(p, 5) * 0.5 + 0.5, bayer(in.position.xy) * hash(p), 1.0);
         }",
    );
    let fx = Fx::new("probe_noise", json!({})).image("left", halves([255; 3], [0; 3]), false);
    let Some(noise) = fx.shot() else { return };
    let spread = |c: usize| {
        let values = noise.0.pixels().map(|p| p.0[c]);
        values.clone().max().unwrap_or(0) - values.min().unwrap_or(0)
    };
    assert!([0, 1, 2].iter().all(|&c| spread(c) > 60), "{:?}", [0, 1, 2].map(spread));
    assert!(noise.mean().iter().all(|m| (20.0..235.0).contains(m)), "{:?}", noise.mean());
    let masked = json!([{ "effect": "color_adjust", "params": { "exposure": -5 }, "mask": "left" }]);
    let Some(out) = Fx::chain(masked).input(solid(BLUE)).image("left", halves([255; 3], [0; 3]), false).shot() else {
        return;
    };
    out.check(&[((10, 16), [0, 0, 50]), ((54, 16), BLUE)]);
}

#[test]
fn lower_quality_keeps_the_output_of_an_effect_sharp() {
    let rays = Fx::new("god_rays", json!({ "threshold": 4 })).input(halves(RED, BLUE)).adjust();
    let Some(out) = rays.quality(Quality::Q2).shot() else { return };
    out.check(&[((31, 16), RED), ((32, 16), BLUE)]);
}

#[test]
fn fused_chains_are_compiled_with_the_view() {
    let compiled = |effects: Value| {
        let (mut run, mut counts) = (Fx::chain(effects).run()?, Vec::new());
        for _ in 0..4 {
            run.shot();
            counts.push(run.stats().compiled);
            run.runtime.advance(0.5);
        }
        Some(counts)
    };
    let Some(plain) = compiled(json!([{ "effect": "vignette" }])) else { return };
    let toggled = json!({ "keys": [[0, true], [0.5, false], [1, true]], "loop": "repeat" });
    for effects in [
        json!([{ "effect": "shine" }, { "effect": "vignette" }, { "effect": "film_grain" }]),
        json!([{ "effect": "posterize" }, { "effect": "vignette", "enabled": false }, { "effect": "chroma_key" }, { "effect": "film_grain" }]),
        json!([{ "effect": "posterize" }, { "effect": "vignette", "enabled": toggled }, { "effect": "chroma_key" }, { "effect": "film_grain" }]),
    ] {
        assert_eq!(compiled(effects.clone()), Some(plain.clone()), "{effects}");
    }
}

#[test]
fn the_last_view_of_a_fused_chain_frees_its_shader() {
    let Some(Run { r, runtime, view, .. }) = Fx::chain(json!([{ "effect": "shine" }, { "effect": "vignette" }])).run()
    else {
        return;
    };
    let held = r.effect_shaders();
    let again = r.scene_view(&runtime.scene, &SceneAssets::default()).unwrap();
    drop(view);
    assert_eq!(r.effect_shaders(), held, "another view still draws the chain");
    drop(again);
    assert_eq!(r.effect_shaders(), held - 1);
}

#[test]
fn every_built_in_effect_renders_with_its_defaults() {
    for d in wprs_scene::builtin() {
        let Some(mut run) = Fx::new(&d.id, json!({})).run() else { return };
        let out = run.shot();
        assert!(run.stats().passes >= d.passes.len() as u32, "{} ran {} passes", d.id, run.stats().passes);
        assert!(out.mean().iter().any(|m| *m > 8.0), "{} drew nothing", d.id);
    }
}
