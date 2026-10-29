use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;

fn state() -> &'static Path {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE
        .get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
            dir
        })
        .path()
}

fn import(project: &mut Project, file: &Path) -> anyhow::Result<()> {
    project.act(&serde_json::from_value::<Action>(json!({"import": {"file": file}})).unwrap()).map(drop)
}

fn bmp(dir: &Path, name: &str, side: u32) -> std::path::PathBuf {
    let mut b = b"BM".to_vec();
    [54 + side * side * 3, 0, 54, 40, side, side].iter().for_each(|v| b.extend(v.to_le_bytes()));
    b.extend(1u16.to_le_bytes());
    b.extend(24u16.to_le_bytes());
    b.extend([0u8; 24]);
    fs::write(dir.join(name), &b).unwrap();
    dir.join(name)
}

fn wav(dir: &Path, name: &str, seconds: u32) -> std::path::PathBuf {
    let data = 8000 * seconds;
    let mut b = b"RIFF".to_vec();
    b.extend((36 + data).to_le_bytes());
    b.extend(b"WAVEfmt ");
    [16u32, 0x0001_0001, 8000, 8000, 0x0008_0001].iter().for_each(|v| b.extend(v.to_le_bytes()));
    b.extend(b"data");
    b.extend(data.to_le_bytes());
    b.resize(b.len() + data as usize, 128);
    fs::write(dir.join(name), &b).unwrap();
    dir.join(name)
}

fn assets(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(dir.join("assets"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    found.sort();
    found
}

#[test]
fn imports_stop_at_the_limits_of_the_format_and_leave_nothing_behind() {
    let (root, _) = (tempfile::tempdir().unwrap(), state());
    let dir = root.path().join("loud");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.loud", "Loud", "en").unwrap()).unwrap();
    for n in 0..3 {
        import(&mut project, &wav(root.path(), &format!("s{n}.wav"), 55)).unwrap();
    }
    let error = import(&mut project, &wav(root.path(), "s3.wav", 20)).unwrap_err().to_string();
    assert!(error.contains("180 s"), "{error}");
    import(&mut project, &wav(root.path(), "song.wav", 240)).unwrap();
    let error = import(&mut project, &wav(root.path(), "opera.wav", 700)).unwrap_err().to_string();
    assert!(error.contains("600 s"), "{error}");
    import(&mut project, &bmp(root.path(), "a.bmp", 9000)).unwrap();
    let error = import(&mut project, &bmp(root.path(), "b.bmp", 9000)).unwrap_err().to_string();
    assert!(error.contains("512 MiB"), "{error}");
    image::RgbaImage::new(2, 2).save(root.path().join("c.png")).unwrap();
    let taken = json!({"import": {"file": root.path().join("c.png"), "id": "a"}});
    let error = project.act(&serde_json::from_value(taken).unwrap()).unwrap_err().to_string();
    assert!(error.contains("the asset a exists"), "{error}");
    assert_eq!(assets(&dir), ["a.bmp", "s0.wav", "s1.wav", "s2.wav", "song.wav"]);
    let many: Value = (0..256).map(|n| (format!("x{n}"), json!({"path": "assets/a.bmp"}))).collect();
    project
        .act(&serde_json::from_value(json!({"set": {"pointer": "/scene/assets", "value": many}})).unwrap())
        .unwrap();
    let error = import(&mut project, &wav(root.path(), "s4.wav", 1)).unwrap_err().to_string();
    assert!(error.contains("at most 256 assets"), "{error}");
}

#[test]
fn recovery_files_go_away_when_nothing_is_left_to_recover() {
    let (root, state) = (tempfile::tempdir().unwrap(), state().join("wallpaper-rs/editor"));
    let dir = root.path().join("quiet");
    let mut project =
        Project::create(&dir, starter("empty", "org.example.quiet", "Quiet", "en").unwrap()).unwrap();
    let files = || fs::read_dir(&state).map_or(0, |d| d.count());
    let before = files();
    project
        .act(
            &serde_json::from_value(
                json!({"set": {"pointer": "/scene/canvas/background", "value": "#101010"}}),
            )
            .unwrap(),
        )
        .unwrap();
    project.persist().unwrap();
    assert_eq!(files(), before + 1);
    project.save().unwrap();
    project.forget().unwrap();
    assert_eq!(files(), before);
    for n in 0..40 {
        fs::write(state.join(format!("old{n}.json")), "{}").unwrap();
    }
    project.persist().unwrap();
    assert!(files() <= 32, "{} recovery files are kept", files());
}
