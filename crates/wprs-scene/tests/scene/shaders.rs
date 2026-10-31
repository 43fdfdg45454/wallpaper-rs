use super::*;

fn compile(source: &str) -> Result<naga::Module, String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let mut validator = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), Default::default());
    validator.validate(&module).map_err(|e| e.emit_to_string(source))?;
    Ok(module)
}

#[test]
fn every_built_in_effect_matches_its_shader() {
    assert!(compile(&format!("{PRELUDE}\nfn broken(")).is_err());
    let base = compile(PRELUDE).unwrap();
    let bindings = |m: &naga::Module| m.global_variables.iter().filter(|(_, g)| g.binding.is_some()).count();
    let ids: Vec<&str> = builtin().iter().map(|d| d.id.as_str()).collect();
    assert!(ids.is_sorted() && ["blur", "color_adjust", "waves", "bloom"].iter().all(|id| ids.contains(id)));
    for d in builtin() {
        assert_eq!(d.problems(), Vec::<String>::new());
        let module = compile(&effect_source(&[d])).unwrap_or_else(|e| panic!("{}: {e}", d.id));
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        for pass in &d.passes {
            assert!(entries.contains(&pass.entry.as_str()), "{} has no entry {}", d.id, pass.entry);
        }
        assert_eq!(bindings(&module), bindings(&base), "{} declares its own bindings", d.id);
    }
    let fused: Vec<&Descriptor> = builtin().iter().filter(|d| d.fuse.is_some()).collect();
    let chain = compile(&effect_source(&fused)).unwrap_or_else(|e| panic!("fused effects clash: {e}"));
    let entries: Vec<&str> =
        base.entry_points.iter().chain(&chain.entry_points).map(|e| e.name.as_str()).collect();
    assert!([CHAIN_ENTRY, "fs_mask_mix", "vs_main"].iter().all(|e| entries.contains(e)), "{entries:?}");
    let blur = descriptor("blur").unwrap();
    let packed = blur.pack(|id| (id == "radius").then_some(Param::Number(300.0)));
    assert_eq!((packed[0], packed[1]), ([128.0, 0.0, 0.0, 0.0], [0.0; 4]));
    let waves = descriptor("waves").unwrap();
    assert_eq!(
        waves.pack(|_| None)[..4],
        [[8.0, 0.0, 0.0, 0.0], [4.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [0.0; 4]]
    );
    assert_eq!(
        [waves.class(), blur.class(), descriptor("bloom").unwrap().class()],
        [Class::Light, Class::Medium, Class::Medium]
    );
    assert!(waves.animates() && !blur.animates());
}

#[test]
fn built_in_effects_compile_and_report_their_cost() {
    use Class::{Light, Medium};
    let expected = [
        ("blur_motion", Medium, 2.0),
        ("blur_radial", Medium, 4.0),
        ("tilt_shift", Medium, 2.25),
        ("god_rays", Medium, 3.0),
        ("shine", Light, 1.0),
        ("color_lut", Light, 1.0),
        ("posterize", Light, 1.0),
        ("chroma_key", Light, 1.0),
        ("vignette", Light, 1.0),
        ("chromatic_aberration", Light, 1.0),
        ("film_grain", Light, 1.0),
        ("crt", Light, 1.0),
        ("pixelate", Light, 1.0),
        ("glitch", Light, 1.0),
        ("edges", Medium, 2.5),
    ];
    for (id, class, units) in expected {
        let d = descriptor(id).unwrap_or_else(|| panic!("{id} is not built in"));
        assert_eq!(d.problems(), Vec::<String>::new());
        compile(&effect_source(&[d])).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!((d.class(), d.units()), (class, units), "{id}");
        assert!(d.uses.is_empty(), "{id} wakes the scene by itself");
    }
    let depth = descriptor("depth_parallax").unwrap();
    compile(&effect_source(&[depth])).unwrap_or_else(|e| panic!("depth_parallax: {e}"));
    assert_eq!((depth.problems(), depth.class(), depth.units()), (Vec::<String>::new(), Medium, 3.0));
    let gates: Vec<Use> = depth.params.iter().filter_map(|p| p.uses).collect();
    assert_eq!((gates, depth.reach.as_deref()), (vec![Use::Cursor, Use::Time], Some("strength")));
    let steps = [Quality::Q1, Quality::Q2, Quality::Q3].map(|q| depth.setting(q, "steps", None));
    assert_eq!(steps, [Some(Param::Number(8.0)), Some(Param::Number(12.0)), None]);
    let samples = |id: &str, q| descriptor(id).and_then(|d| d.tuned(q, "samples")).cloned();
    assert_eq!(samples("god_rays", Quality::Q1), Some(Param::Number(8.0)));
    assert_eq!(samples("blur_motion", Quality::Q2), None);
    let fused = ["shine", "posterize", "chroma_key", "vignette", "film_grain", "pixelate"].map(descriptor);
    let fused: Vec<&Descriptor> = fused.into_iter().flatten().collect();
    assert!(fused.len() == 6 && fused.iter().all(|d| d.fuse.is_some()));
    compile(&effect_source(&fused)).unwrap_or_else(|e| panic!("fused chain: {e}"));
}
