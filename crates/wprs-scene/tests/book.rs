use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use wprs_scene::*;

static ONE: Mutex<()> = Mutex::new(());

const DEAD: &str = "4000000000.1";
const KEY: u64 = 0x4b;

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(String::from).collect()
}

fn fresh() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stopped-effects");
    keep_strikes(&path);
    (dir, path)
}

#[test]
fn blocked_effects_stay_blocked_however_many_others_are_struck() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let (_dir, path) = fresh();
    strike(&[KEY]).unwrap();
    strike(&[KEY]).unwrap();
    let fillers: Vec<u64> = (0x1000..0x1000 + 64).collect();
    strike(&fillers).unwrap();
    keep_strikes(&path);
    assert!(struck(KEY), "a crash beside 64 other effects forgot a blocked one");
    for others in [96u64, 128] {
        let (_dir, path) = fresh();
        let drawn: Vec<u64> = std::iter::once(KEY).chain(0x2000..0x2000 + others).collect();
        for crash in 1..=12 {
            strike(&drawn).unwrap();
            keep_strikes(&path);
            assert_eq!(struck(KEY), crash >= 2, "crash {crash} beside {others} others");
        }
    }
    let (_dir, path) = fresh();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let many: String = (0..4100u64).map(|k| format!("{k:016x} 2 {}\n", now - 4100 + k)).collect();
    std::fs::write(&path, many).unwrap();
    keep_strikes(&path);
    strike(&[]).unwrap();
    assert_eq!(lines(&path).len(), 4096);
    assert!(!struck(3) && struck(4) && struck(4099), "only the oldest blocked lines past the cap go");
}

#[test]
fn tampered_counts_neither_panic_nor_wrap_to_unblocked() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let (_dir, path) = fresh();
    std::fs::write(&path, format!("0000000000000011 4294967295 0\n0000000000000011 trying {DEAD}\n"))
        .unwrap();
    keep_strikes(&path);
    assert!(struck(0x11));
    strike(&[]).unwrap();
    assert!(struck(0x11));
    assert_eq!(lines(&path).len(), 1);
    assert!(lines(&path)[0].starts_with("0000000000000011 2 "), "{:?}", lines(&path));
}

fn scene() -> (Scene, impl Fn(&str, usize) -> Result<Vec<u8>, String> + Copy) {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "assets": { "a": { "path": "a.png" } },
        "effects": { "s": { "descriptor": "fx/s.json", "wgsl": "fx/s.wgsl" } },
        "layers": [{ "id": "l", "type": "image", "image": "a", "effects": [{ "effect": "s" }] }] });
    let files = |path: &str, _: usize| match path {
        "fx/s.json" => {
            Ok(json!({ "id": "s", "cost": {}, "passes": [{ "entry": "fs_s" }] }).to_string().into_bytes())
        }
        "fx/s.wgsl" => {
            Ok(b"@fragment fn fs_s(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * 0.5; }"
                .to_vec())
        }
        _ => Err("missing".to_string()),
    };
    (Scene::load(&text.to_string(), &BTreeMap::new()).unwrap(), files)
}

#[test]
fn retrying_effects_undoes_a_block_found_while_checking_a_package() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let (_dir, path) = fresh();
    let (mut probe, files) = scene();
    let (json, wgsl) = (files("fx/s.json", 0).unwrap(), files("fx/s.wgsl", 0).unwrap());
    let (json, wgsl) = (String::from_utf8(json).unwrap(), String::from_utf8(wgsl).unwrap());
    let checked = Descriptor::packaged("s", &json, &wgsl).unwrap().key & !1;
    std::fs::write(&path, format!("{checked:016x} 2 1\n")).unwrap();
    keep_strikes(&path);
    let fault = probe.adopt(&BTreeMap::new(), files).unwrap_err().to_string();
    assert!(fault.contains("not checked again") && fault.contains(STOPPED), "{fault}");
    assert_eq!(forgive_strikes().unwrap(), 1);
    let (mut again, files) = scene();
    again.adopt(&BTreeMap::new(), files).unwrap();
}
