use super::*;

#[test]
fn values_of_described_parameters_are_named_in_english_and_spanish() {
    let newer = builtin().iter().flat_map(|b| newer_params(&b.id));
    let all = newer.chain(undrawn().iter().chain(builtin()).flat_map(|d| &d.params));
    let labels: Vec<_> = all.flat_map(|p| &p.values).map(|e| &e.label).collect();
    assert!(labels.len() > 20 && labels.iter().all(|l| l.contains_key("en") && l.contains_key("es")));
}

const BASE: &str = "bloom: threshold intensity radius knee; blur: radius; blur_motion: angle length samples;
    blur_radial: center strength samples follow; chroma_key: key tolerance softness spill;
    chromatic_aberration: amount center; clouds: color shade opacity coverage softness size speed warp octaves height;
    color_adjust: exposure contrast saturation hue; color_lut: lut intensity; crt: curvature pitch scan phosphor vignette glow;
    depth_parallax: depth strength focus steps overscan follow orbit; dissolve: progress size edge glow;
    edges: strength threshold width color sketch paper; film_grain: amount size fps; flow: map strength speed direction;
    glitch: bursts beat length rows threshold shift split jitter speed;
    god_rays: source threshold density decay exposure color samples follow; iris: range follow center saccades rate;
    lens_distort: k1 k2 fit center; perspective: top_left top_right bottom_right bottom_left; pixelate: size hex;
    posterize: levels dither; reflection: line opacity fade amplitude frequency speed; scroll: speed;
    shake: strength speed direction; shine: angle width softness intensity color speed gap phase;
    sway: amplitude speed stiffness spread swing angle pivot; tilt_shift: focus width softness angle radius depth;
    twirl: angle radius center; vignette: intensity radius softness roundness center color;
    water_rings: amplitude wavelength speed falloff center; water_ripple: strength damping radius cursor click beat specular;
    waves: amplitude frequency speed direction";

#[test]
fn parameters_added_to_built_in_effects_are_declared_as_newer() {
    let base: BTreeMap<&str, Vec<&str>> = BASE
        .split(';')
        .filter_map(|e| e.split_once(':'))
        .map(|(id, params)| (id.trim(), params.split_whitespace().collect()))
        .collect();
    assert_eq!(base.len(), 33);
    for b in builtin() {
        let Some(known) = base.get(b.id.as_str()) else { continue };
        for p in &b.params {
            let newer = newer_params(&b.id).any(|n| n.id == p.id);
            assert!(
                known.contains(&p.id.as_str()) || newer,
                "{}.{} needs an entry in catalog.json",
                b.id,
                p.id
            );
        }
    }
}

#[test]
fn described_effects_are_complete_and_drawn_only_with_a_shader() {
    assert!(undrawn().is_empty());
    for id in NEWER.split_whitespace() {
        assert!(drawn(id, None) && descriptor(id).is_some_and(|d| d.problems().is_empty()), "{id}");
    }
    for d in undrawn() {
        let problems: Vec<String> =
            d.problems().into_iter().filter(|p| !p.contains("a fused effect needs")).collect();
        assert!(problems.is_empty(), "{problems:?}");
        assert!(d.name.contains_key("en") && reserved(&d.id) && !drawn(&d.id, None), "{}", d.id);
        assert!(descriptor(&d.id).is_none() && described(&d.id) == Some(d), "{}", d.id);
    }
    for b in builtin() {
        let more: Vec<&ParamDef> = newer_params(&b.id).collect();
        let full = described(&b.id).unwrap();
        for p in &more {
            let shown: Vec<&ParamDef> = full.params.iter().filter(|o| o.id == p.id).collect();
            assert!(
                p.accepts(&p.default) && matches!(shown[..], [o] if o.kind == p.kind),
                "{}.{}",
                b.id,
                p.id
            );
            assert_eq!(drawn(&b.id, Some(&p.id)), b.params.iter().any(|o| o.id == p.id), "{}.{}", b.id, p.id);
        }
        assert!(full.params.iter().all(|p| b.params.contains(p) || more.contains(&p)));
        assert!(descriptor(&b.id) == Some(b) && drawn(&b.id, b.params.first().map(|p| p.id.as_str())));
    }
    assert!(
        newer_params("waves").any(|p| p.id == "phase") && newer_params("god_rays").any(|p| p.id == "edges")
    );
}

#[test]
fn described_effects_and_parameters_are_validated_like_built_in_ones() {
    let ok = effect(json!({ "effect": "light_shafts", "params": { "intensity": 2, "gradient": "sky" } }));
    assert_eq!(issues(ok), Vec::<String>::new());
    assert_eq!(
        issues(effect(json!({ "effect": "blur", "params": { "blend": "screen", "amount": 0.5 } }))),
        Vec::<String>::new()
    );
    for picture in wprs_scene::GENERIC_PICTURES {
        let named = effect(json!({ "effect": "reveal", "params": { "texture": picture } }));
        let bound = json!({ "prop": "look", "map": { "a": picture, "b": "sky" } });
        assert_eq!(issues(named), Vec::<String>::new(), "{picture}");
        let mapped = effect(json!({ "effect": "reveal", "params": { "texture": bound } }));
        assert!(issues(mapped).iter().all(|i| !i.contains("asset")), "{picture}");
    }
    let cases = [
        (
            json!({ "effect": "light_shafts", "params": { "intensity": 9 } }),
            "/layers/0/effects/0/params/intensity is 9, expected 0..=4",
        ),
        (
            json!({ "effect": "blur", "params": { "blend": "sparkle" } }),
            "/layers/0/effects/0/params/blend does not fit",
        ),
        (
            json!({ "effect": "god_rays", "params": { "edges": 2.5 } }),
            "/layers/0/effects/0/params/edges does not fit",
        ),
        (
            json!({ "effect": "tint", "params": { "mode": 4 } }),
            "/layers/0/effects/0/params/mode does not fit",
        ),
        (json!({ "effect": "reveal", "params": { "texture": "moon" } }), "names the unknown asset \"moon\""),
        (json!({ "effect": "blur", "params": { "phase": "sky" } }), "names the unknown parameter \"phase\""),
    ];
    for (e, wanted) in cases {
        let found = issues(effect(e.clone()));
        assert!(found.iter().any(|f| f.contains(wanted)), "{e}: {found:?}");
    }
    let mut own = scene(
        json!({ "effects": { "tint": { "descriptor": "effects/tint.json", "wgsl": "effects/tint.wgsl" } } }),
    );
    let found =
        own.adopt(&BTreeMap::new(), |_: &str, _| Ok::<_, String>(Vec::new())).unwrap_err().to_string();
    assert!(found.contains("/effects/tint"), "{found}");
    let follow = scene(effect(json!({ "effect": "reveal", "params": { "follow": true } })));
    let beat = scene(effect(json!({ "effect": "pulse", "params": { "audio_band": 3 } })));
    let still = scene(effect(json!({ "effect": "pulse" })));
    assert!(needs(&follow).cursor && needs(&beat).audio && !needs(&still).audio && !needs(&still).cursor);
}

#[test]
fn a_package_effect_use_reads_a_generic_picture_by_name() {
    let descriptor = json!({ "id": "phased", "cost": {}, "params": [
        { "id": "phase", "type": "texture", "default": "" },
        { "id": "flat", "type": "texture", "default": "normal_flat" }],
        "passes": [{ "entry": "fs_phased", "inputs": ["previous", "asset:phase", "asset:flat"] }] });
    let wgsl = "@fragment fn fs_phased(in: VertexOut) -> @location(0) vec4<f32> {
    return sample(aux0, in.uv) * sample(aux1, in.uv);
}";
    let validated = |phase: &str| {
        let used = effect(json!({ "effect": "phased", "params": { "phase": phase } }));
        let mut doc =
            scene(json!({ "effects": { "phased": { "descriptor": "fx/p.json", "wgsl": "fx/p.wgsl" } },
            "layers": used["layers"] }));
        let files = |path: &str, _| {
            Ok::<_, String>(
                if path.ends_with("json") { descriptor.to_string() } else { wgsl.into() }.into_bytes(),
            )
        };
        let adopted = doc.adopt(&BTreeMap::new(), files).err().map(|e| e.to_string());
        let found = doc.validate(&BTreeMap::new()).iter().map(ToString::to_string).collect::<Vec<_>>();
        adopted.into_iter().chain(found).collect::<Vec<_>>()
    };
    assert_eq!(validated("white"), Vec::<String>::new());
    assert_eq!(validated("noise"), Vec::<String>::new());
    let unknown = validated("moon");
    assert!(unknown.iter().any(|i| i.contains("/layers/0/effects/0/params/phase")), "{unknown:?}");
}
