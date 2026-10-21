mod common;

use common::{entry, files, manifest};
use wprs_format::{Error, FileEntry, Kind, Limits, Manifest, Result, check_path, validate};

fn run(manifest: &Manifest, files: &[FileEntry]) -> Result<()> {
    validate(manifest, files, &Limits::default())
}

fn with_file(kind: Kind, file: FileEntry) -> Result<()> {
    let manifest = manifest(kind);
    run(&manifest, &[files(&manifest), vec![file]].concat())
}

#[test]
fn valid_file_lists_pass() {
    for kind in [Kind::Image, Kind::Video, Kind::Scene] {
        let manifest = manifest(kind);
        run(&manifest, &files(&manifest)).unwrap();
    }
    with_file(Kind::Scene, entry("effects/glow.wgsl", 10)).unwrap();
    with_file(Kind::Image, entry("..a.png", 10)).unwrap();
}

#[test]
fn unsafe_paths_are_rejected() {
    let long = "a".repeat(252) + ".png";
    let unsafe_paths = ["../evil.png", "a/../../evil.png", "/etc/passwd.png", "C:/evil.png", "C:\\evil.png"];
    let malformed =
        ["a\\b.png", "a//b.png", "./a.png", "a/./b.png", "assets/", "", "a\0b.png", "a\nb.png", &long];
    for path in unsafe_paths.into_iter().chain(malformed) {
        assert!(matches!(check_path(path), Err(Error::UnsafePath(_))), "{path:?}");
        assert!(matches!(with_file(Kind::Image, entry(path, 1)), Err(Error::UnsafePath(_))), "{path:?}");
    }
    for path in ["image.png", "assets/a.png", "a..b/c.png", "dir.d/x.png", &long[1..]] {
        check_path(path).unwrap();
    }
}

#[test]
fn referenced_files_must_exist_and_fit_the_kind() {
    let manifest = manifest(Kind::Image);
    for path in ["image.png", "preview.webp", "manifest.json"] {
        let list: Vec<_> = files(&manifest).into_iter().filter(|f| f.path != path).collect();
        assert!(matches!(run(&manifest, &list), Err(Error::Missing(p)) if p == path), "{path}");
    }
    let mut escaping = manifest.clone();
    escaping.entry = "../image.png".into();
    assert!(matches!(run(&escaping, &files(&manifest)), Err(Error::UnsafePath(_))));
    let mut video = common::manifest(Kind::Video);
    let list = [files(&video), vec![entry("still.png", 10)]].concat();
    video.entry = "still.png".into();
    assert!(matches!(run(&video, &list), Err(Error::Invalid { field, .. }) if field == "entry"));
    let mut scene = common::manifest(Kind::Scene);
    let list = files(&scene);
    scene.preview = Some("scene.json".into());
    assert!(matches!(run(&scene, &list), Err(Error::Invalid { field, .. }) if field == "preview"));
    scene.preview = None;
    scene.entry = "manifest.json".into();
    assert!(matches!(run(&scene, &list), Err(Error::Invalid { field, .. }) if field == "entry"));
}

#[test]
fn extensions_depend_on_the_kind() {
    for path in ["effects/glow.wgsl", "LICENSE", "run.sh", "clip.webm"] {
        let result = with_file(Kind::Image, entry(path, 10));
        assert!(matches!(result, Err(Error::Extension { kind: Kind::Image, .. })), "{path}: {result:?}");
    }
    with_file(Kind::Video, entry("clip.WEBM", 10)).unwrap();
    assert!(matches!(with_file(Kind::Scene, entry("tool.exe", 10)), Err(Error::Extension { .. })));
}

#[test]
fn duplicates_and_symlinks_are_rejected() {
    for path in ["image.png", "IMAGE.png", "Preview.WEBP"] {
        assert!(matches!(with_file(Kind::Image, entry(path, 10)), Err(Error::Duplicate(_))), "{path}");
    }
    let link = FileEntry { symlink: true, ..entry("assets/link.png", 10) };
    assert!(matches!(with_file(Kind::Image, link), Err(Error::Symlink(_))));
}

#[test]
fn limits_are_enforced() {
    let manifest = manifest(Kind::Image);
    let list = files(&manifest);
    let limited = |limits: Limits| validate(&manifest, &list, &limits);
    let defaults = Limits::default();
    let cases = [
        (Limits { max_entries: 2, ..defaults }, "entry count"),
        (Limits { max_file_size: 999, ..defaults }, "file size"),
        (Limits { max_total_size: 1399, ..defaults }, "total size"),
    ];
    for (limits, name) in cases {
        assert!(matches!(limited(limits), Err(Error::Limit { what, .. }) if what == name), "{name}");
    }
    limited(Limits { max_entries: 3, max_file_size: 1000, max_total_size: 1400, ..defaults }).unwrap();
}

#[test]
fn compression_ratio_is_bounded() {
    let bomb = FileEntry { compressed_size: 64 << 10, ..entry("assets/bomb.png", 64 << 20) };
    let result = with_file(Kind::Image, bomb);
    assert!(matches!(result, Err(Error::Limit { what: "compression ratio", actual: 1024, limit: 100 })));
    with_file(Kind::Image, FileEntry { compressed_size: 1, ..entry("assets/small.png", 1 << 20) }).unwrap();
    with_file(Kind::Image, FileEntry { compressed_size: 0, ..entry("assets/empty.png", 0) }).unwrap();
}
