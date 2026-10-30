use serde_json::json;
use std::path::Path;
use wprs_scene::*;

const DEAD: &str = "4000000000.1";

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(String::from).collect()
}

fn effect() -> (String, String) {
    let json = json!({ "id": "s", "cost": {}, "passes": [{ "entry": "fs_s" }] }).to_string();
    (json, "@fragment fn fs_s(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }".into())
}

fn heads(path: &Path) -> Vec<String> {
    lines(path).iter().map(|l| l.split_whitespace().take(2).collect::<Vec<_>>().join(" ")).collect()
}

#[test]
fn crashes_hangs_and_losses_count_strikes_that_block_an_effect_until_it_is_forgiven() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache").join("stopped-effects");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let (json, wgsl) = effect();
    let key = Descriptor::packaged("s", &json, &wgsl).unwrap().key;
    let checked = key & !1;
    keep_strikes(&path);
    assert!(!struck(key) && !path.exists(), "nothing is read or written before it is needed");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("{checked:016x} trying {DEAD}\n{key:016x} 1 {now}\n")).unwrap();
    keep_strikes(&path);
    assert!(!struck(checked) && !struck(key), "one crash or loss is one strike");
    Descriptor::packaged("s", &json, &wgsl).unwrap();
    assert_eq!(heads(&path), [format!("{key:016x} 1"), format!("{checked:016x} 1")]);
    std::fs::write(&path, format!("{checked:016x} 1 {now}\n{checked:016x} trying {DEAD}\n")).unwrap();
    keep_strikes(&path);
    let fault = Descriptor::packaged("s", &json, &wgsl).unwrap_err();
    assert!(fault.contains("not checked again") && fault.contains("effects again"), "{fault}");
    assert_eq!(forgive_strikes().unwrap(), 1);
    assert!(!struck(checked) && lines(&path).is_empty());
    Descriptor::packaged("s", &json, &wgsl).unwrap();
    trying(key).unwrap();
    assert_eq!(heads(&path), [format!("{key:016x} trying")]);
    keep_strikes(&path);
    assert!(!struck(key), "the trial of a running process is no strike");
    tried(key);
    assert!(lines(&path).is_empty());
    trying(key).unwrap();
    strike(&[key]).unwrap();
    strike(&[key]).unwrap();
    assert!(struck(key) && trying(key).is_err());
    assert_eq!(heads(&path), [format!("{key:016x} 2")]);
    std::fs::write(&path, format!("{key:016x} 1 {}\n", now - FORGIVEN - 1)).unwrap();
    keep_strikes(&path);
    strike(&[]).unwrap();
    assert!(lines(&path).is_empty(), "a single strike is forgiven after a day");
    std::fs::write(&path, (0..1100u64).map(|k| format!("{k:016x} 1 {now}\n")).collect::<String>()).unwrap();
    keep_strikes(&path);
    strike(&[]).unwrap();
    let kept = heads(&path);
    assert_eq!((kept.len(), kept[0].as_str()), (1024, format!("{:016x} 1", 76).as_str()));
}
