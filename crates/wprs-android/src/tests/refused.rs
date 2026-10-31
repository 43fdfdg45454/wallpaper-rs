use crate::store;
use std::fs::File;

mod table {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/features.rs"));
}

pub(super) fn packages_the_phone_cannot_play_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut cases = table::cases();
    cases.push(("effects".into(), table::effects()));
    let mut stored = 0;
    for (name, case) in &cases {
        let path = table::build(name, case, dir.path()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let package = wprs_format::open(&path, &wprs_format::Limits::default()).unwrap();
        let refused = wprs_format::features::refused("android", &package.facts.features);
        let result = store::store(&mut File::open(&path).unwrap(), &format!("{name}.wpak"));
        let error = result.as_ref().err().map(ToString::to_string).unwrap_or_default();
        if refused.is_empty() {
            assert!(!error.contains("needs a newer version"), "{name}: {error}");
            let scene = package.manifest.kind == wprs_format::Kind::Scene;
            assert_eq!(result.is_ok(), scene, "{name}: {error}");
            stored += usize::from(scene);
        } else {
            assert!(
                error.starts_with("this package needs a newer version of wallpaper-rs"),
                "{name}: {error}"
            );
            for feature in &refused {
                let why = format!("{feature} ({})", wprs_format::features::explain(feature));
                assert!(error.contains(&why), "{name}: {error}");
            }
        }
    }
    assert!(stored > 10, "{stored}");
    let mut desktop = table::cases().into_iter().find(|(n, _)| n == "platforms").unwrap().1;
    desktop["manifest"]["platforms"] = serde_json::json!(["desktop"]);
    let path = table::build("desktop_only", &desktop, dir.path()).unwrap();
    let error = store::store(&mut File::open(&path).unwrap(), "desktop.wpak").unwrap_err().to_string();
    assert!(error.contains("made for the desktop only"), "{error}");
}

#[test]
fn every_feature_the_phone_leaves_out_has_a_note_on_its_page() {
    let detail = include_str!("../../../../android/app/src/main/kotlin/Detail.kt");
    let res = concat!(env!("CARGO_MANIFEST_DIR"), "/../../android/app/src/main/res");
    let strings =
        ["values", "values-es"].map(|v| std::fs::read_to_string(format!("{res}/{v}/strings.xml")).unwrap());
    let left = wprs_format::features::all().filter(|(.., on)| {
        on[wprs_format::Platform::Android as usize] == wprs_format::features::Support::Without
    });
    for (name, ..) in left {
        assert!(
            detail.contains(&format!("\"{name}\" -> R.string.lacks_{name}\n")),
            "Detail.kt has no note for {name}"
        );
        for text in &strings {
            assert!(text.contains(&format!("<string name=\"lacks_{name}\">")), "no lacks_{name} string");
        }
    }
}

#[test]
fn refusals_are_told_in_the_language_of_the_phone() {
    let main = include_str!("../../../../android/app/src/main/kotlin/Main.kt");
    let res = concat!(env!("CARGO_MANIFEST_DIR"), "/../../android/app/src/main/res");
    let strings =
        ["values", "values-es"].map(|v| std::fs::read_to_string(format!("{res}/{v}/strings.xml")).unwrap());
    let constant = |name: &str| {
        let line = main.lines().find(|l| l.starts_with(&format!("private const val {name} = \""))).unwrap();
        line.split('"').nth(1).unwrap().to_owned()
    };
    let errors = [
        ("NEWER", "refused_newer", wprs_format::Error::Newer("the feature web".into())),
        ("ELSEWHERE", "refused_elsewhere", wprs_format::Error::Elsewhere("the desktop".into())),
    ];
    for (name, key, error) in errors {
        assert!(error.to_string().starts_with(&constant(name)), "{name} no longer matches {error}");
        assert!(main.contains(&format!("startsWith({name}) -> getString(R.string.{key})")));
        for text in &strings {
            assert!(text.contains(&format!("<string name=\"{key}\">")), "no {key} string");
        }
    }
}

#[test]
fn an_error_without_text_is_told_with_a_translated_reason() {
    let main = include_str!("../../../../android/app/src/main/kotlin/Main.kt");
    assert!(main.contains("text.isBlank() -> getString(R.string.failed_without_reason)"));
    let res = concat!(env!("CARGO_MANIFEST_DIR"), "/../../android/app/src/main/res");
    for v in ["values", "values-es"] {
        let text = std::fs::read_to_string(format!("{res}/{v}/strings.xml")).unwrap();
        assert!(text.contains("<string name=\"failed_without_reason\">"), "no reason in {v}");
    }
}

#[test]
fn format_errors_are_told_in_the_language_of_the_phone() {
    let error = crate::Error::Format(wprs_format::Error::Missing("scene.json".into()));
    assert_eq!(error.told_in(&[]), "missing file \"scene.json\"");
    let spanish = error.told_in(&["es_ES".into()]);
    assert!(spanish.contains("scene.json") && !spanish.contains("missing"), "{spanish}");
}
