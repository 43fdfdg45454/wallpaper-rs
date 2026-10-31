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
            let shown = package.manifest.kind.first();
            assert_eq!(result.is_ok(), shown, "{name}: {error}");
            stored += usize::from(shown);
            if let Ok((uid, _)) = result.as_ref()
                && package.manifest.kind != wprs_format::Kind::Scene
            {
                kinds(uid, package.manifest.kind);
            }
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
    unpacked_in_pieces(dir.path());
}

fn unpacked_in_pieces(dir: &std::path::Path) {
    let case = serde_json::json!({ "kind": "video", "entry": "clip.webm", "files": { "clip.webm": "fixture:magenta-64x32.webm" } });
    let source = table::source("long", &case, dir);
    let clip = source.join("clip.webm");
    File::options().append(true).open(&clip).unwrap().set_len(BIG).unwrap();
    let built = dir.join("long.wpak");
    wprs_format::build(&source, &built, &wprs_format::Limits::default()).unwrap();
    let (uid, _) = store::store(&mut File::open(&built).unwrap(), "long.wpak").unwrap();
    let item = crate::library::with(|lib| Ok(lib.item(&uid)?)).unwrap();
    std::fs::remove_dir_all(crate::pack::folder(&item.path).unwrap()).unwrap();
    crate::counted::watch(true);
    let entry = crate::pack::entry(&item.path).unwrap();
    let (_, bytes) = crate::counted::watch(false);
    assert_eq!(std::fs::read(&entry).unwrap(), std::fs::read(&clip).unwrap());
    assert!(bytes < BIG / 4, "unpacking a {BIG} byte video took {bytes} bytes of memory");
    crate::store::forget(&item).unwrap();
}

const BIG: u64 = 16 << 20;

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
    let words = include_str!("../../../../android/app/src/main/kotlin/Words.kt");
    assert!(words.contains("\"error_\" + it.replace('.', '_')"));
    let errors =
        [wprs_format::Error::Newer("the feature web".into()), wprs_format::Error::Elsewhere("x".into())];
    for (error, key) in errors.into_iter().zip(["error_format_newer", "error_format_elsewhere"]) {
        let reply = crate::Error::Format(error).reply();
        assert_eq!(format!("error_{}", reply["id"].as_str().unwrap().replace('.', "_")), key);
    }
}

#[test]
fn an_error_without_text_is_told_with_a_translated_reason() {
    let main = include_str!("../../../../android/app/src/main/kotlin/Main.kt");
    assert!(main.contains("text.ifBlank { getString(R.string.failed_without_reason) }"));
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

fn kinds(uid: &str, kind: wprs_format::Kind) {
    let item = crate::library::with(|lib| Ok(lib.item(uid)?)).unwrap();
    let kept = std::fs::read_dir(crate::pack::folder(&item.path).unwrap()).unwrap().count();
    assert_eq!(kept, 1, "a {kind} package is unpacked when it is imported");
    let (file, shown) = crate::pack::media(&item).unwrap();
    assert_eq!(shown, kind.name(), "a {kind} package shows as one");
    assert!(file.file_stem().is_some_and(|s| s == "entry") && file.is_file(), "{file:?}");
    match kind {
        wprs_format::Kind::Image => assert_eq!(item.meta.width, Some(64)),
        _ => assert!(crate::media::is_video(&file)),
    }
    assert_eq!(crate::store::poster_source(uid).unwrap(), None, "a package preview is its poster");
    crate::store::forget(&item).unwrap();
    assert!(!file.exists());
}
