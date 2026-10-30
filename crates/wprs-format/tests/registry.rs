#[path = "../../../tests/fixtures/features.rs"]
mod table;

use std::collections::BTreeSet;
use wprs_format::features::{self, Support};
use wprs_format::{Limits, Platform, open};

fn outcome(package: &wprs_format::Package<impl std::io::Read + std::io::Seek>, platform: Platform) {
    let refused = features::refused(platform.name(), &package.facts.features);
    match features::playable(&package.manifest, &package.facts, platform) {
        Ok(()) => assert!(refused.is_empty(), "{platform:?} plays {refused:?}"),
        Err(e) => {
            let text = e.to_string();
            assert!(
                text.starts_with("this package needs a newer version of wallpaper-rs: it uses the feature")
            );
            for f in &refused {
                assert!(text.contains(&format!("{f} ({})", features::explain(f))), "{text}");
            }
            assert!(!refused.is_empty(), "{text}");
        }
    }
}

#[test]
fn every_feature_has_a_fixture_that_pack_build_names_and_each_platform_handles_as_listed() {
    let dir = tempfile::tempdir().unwrap();
    let cases = table::cases();
    let named: BTreeSet<&str> = cases.iter().map(|(n, _)| n.as_str()).collect();
    let missing: Vec<&str> = features::all().map(|(n, ..)| *n).filter(|n| !named.contains(n)).collect();
    assert!(missing.is_empty(), "no fixture for {missing:?}");
    for (name, case) in &cases {
        let path = table::build(name, case, dir.path()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let package = open(&path, &Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(package.facts.features.contains(name), "{name}: {:?}", package.facts.features);
        assert!(package.manifest.features().contains(name), "pack build did not list {name}");
        assert!(features::known(name) && !features::explain(name).is_empty(), "{name}");
        for platform in Platform::ALL {
            outcome(&package, platform);
            let refused = features::refused(platform.name(), [name]).contains(name);
            assert_eq!(refused, features::support(platform, name) == Support::Refused, "{name}");
            let lacks = features::lacks(platform.name(), [name]).contains(name);
            assert_eq!(lacks, features::support(platform, name) == Support::Without, "{name}");
        }
    }
}

#[test]
fn every_described_effect_and_newer_parameter_is_named_and_refused_until_it_is_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let path = table::build("effects", &table::effects(), dir.path()).unwrap();
    let package = open(&path, &Limits::default()).unwrap();
    let mut wanted: Vec<String> = table::newer_effects().map(|id| format!("effect.{id}")).collect();
    assert!(
        ["tint", "blend_texture", "light_shafts"].iter().all(|id| wanted.contains(&format!("effect.{id}")))
    );
    for d in wprs_scene::builtin() {
        wanted.extend(wprs_scene::newer_params(&d.id).map(|p| format!("effect.{}.{}", d.id, p.id)));
    }
    assert!(wanted.len() > 50, "{wanted:?}");
    for name in &wanted {
        assert!(package.facts.features.contains(name), "{name}");
        assert!(features::known(name), "{name}");
        let rest = &name["effect.".len()..];
        let (id, param) = rest.split_once('.').map_or((rest, None), |(i, p)| (i, Some(p)));
        let wanted = if wprs_scene::drawn(id, param) { Support::Runs } else { Support::Refused };
        for platform in Platform::ALL {
            assert_eq!(features::support(platform, name), wanted, "{name}");
        }
    }
    assert_eq!(features::explain("effect.blur.blend"), "sets blend on the blur effect");
    assert_eq!(features::explain("effect.light_shafts"), "uses the light_shafts effect");
    for unknown in
        ["effect.blur.radius", "effect.blur.nothing", "effect.nothing", "effect.light_shafts.angle"]
    {
        assert!(!features::known(unknown), "{unknown}");
    }
    for platform in Platform::ALL {
        outcome(&package, platform);
    }
}

#[test]
fn hdr_videos_get_a_facet_and_a_warning_wherever_their_colors_are_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut mp4 = b"\0\0\0\x18ftypisom\0\0\0\0isomiso2".to_vec();
    mp4.resize(9 << 20, 0);
    mp4.extend(b"\0\0\0\x13colrnclx\0\x09\0\x10\0\x09\0");
    let mkv = [&[0x1a, 0x45, 0xdf, 0xa3, 0x80][..], &[0x55, 0xb0, 0x84, 0x55, 0xba, 0x81, 0x12]].concat();
    let encode = |b: &[u8]| {
        format!("base64:{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b))
    };
    let cases = [
        serde_json::json!({ "kind": "video", "entry": "glow.mp4", "files": { "glow.mp4": encode(&mp4) } }),
        serde_json::json!({ "scene": { "assets": { "sky": { "path": "sky.png" }, "clip": { "path": "clip.mkv" } },
            "layers": [{ "id": "clip", "type": "video", "video": "clip" }] }, "files": { "clip.mkv": encode(&mkv) } }),
    ];
    for (i, case) in cases.iter().enumerate() {
        let name = format!("hdr{i}");
        let path = table::build(&name, case, dir.path()).unwrap();
        let mut package = open(&path, &Limits::default()).unwrap();
        assert!(!package.facts.facets.contains(&"hdr".to_owned()), "opening a package reads no video");
        package.look_at_videos().unwrap();
        assert!(package.facts.facets.contains(&"hdr".to_owned()));
        let (manifest, facts) = wprs_format::inspect(dir.path().join(&name)).unwrap();
        let warned = features::warnings(&manifest, &facts);
        assert!(warned.iter().any(|w| w.contains("is an HDR video")), "{warned:?}");
    }
    let plain = table::cases().into_iter().find(|(n, _)| n == "video_properties").unwrap().1;
    let path = table::build("plain", &plain, dir.path()).unwrap();
    let mut package = open(&path, &Limits::default()).unwrap();
    package.look_at_videos().unwrap();
    assert!(!package.facts.facets.contains(&"hdr".to_owned()));
    assert!(!wprs_format::media::hdr(&mp4[..4 << 20]) && wprs_format::media::hdr(&mkv));
}
