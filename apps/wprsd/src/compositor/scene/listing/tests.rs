use super::super::super::SceneSpec;
use super::super::super::power::Power;
use super::*;
use crate::scenes::ScenePack;
use serde_json::json;
use wprs_scene::{Scene, Tier};

fn state(folder: Option<&std::path::Path>) -> SceneState {
    let scene = json!({ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [16, 9] },
        "scripts": [{ "path": "main.js", "uses": ["folders"] }],
        "layers": [{ "id": "photo", "type": "solid", "color": "#ffffff" }] });
    let scene = Scene::from_json(&scene.to_string()).unwrap();
    let properties = serde_json::from_value(
        json!({ "album": { "type": "folder", "mode": "all", "accept": ["image", "video"] } }),
    )
    .unwrap();
    let pack = Arc::new(ScenePack::bare(scene, properties, Vec::new()));
    let chosen = folder.map(|f| json!({ "album": f }));
    let values = serde_json::from_value(chosen.unwrap_or(json!({}))).unwrap();
    let spec = SceneSpec {
        values,
        pack,
        extra: Vec::new(),
        clips: Vec::new(),
        hold: None,
        moving: false,
        quality: None,
        span: None,
    };
    SceneState::new(spec, Power::new(Tier::T3, false, 0))
}

fn listed(state: &mut SceneState, at: Instant) -> bool {
    (0..200).any(|_| state.listing(at) || (std::thread::sleep(Duration::from_millis(5)), false).1)
}

#[test]
fn a_still_scene_looks_at_its_folder_off_the_compositor_and_wakes_for_it() {
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(dir.path().join("a.png")).unwrap();
    let (mut state, start) = (state(Some(dir.path())), Instant::now());
    assert!(!state.listing(start) && state.listing_wake(start) == Wake::At(SOON.as_secs_f64()));
    assert!(listed(&mut state, start), "the first listing never arrived");
    assert_eq!(state.listed.text.matches("a.png").count(), 1);
    assert_eq!(state.listing_wake(start), Wake::At(EVERY.as_secs_f64()));
    assert!(!state.listing(start + Duration::from_secs(1)), "a second look before 2 s");
    image::RgbaImage::new(2, 2).save(dir.path().join("b.png")).unwrap();
    let later = start + EVERY;
    assert!(listed(&mut state, later), "a new file was not seen");
    assert!(state.listed.text.contains("b.png"));
    let again = later + EVERY;
    assert!(!listed(&mut state, again), "an unchanged folder counted as a change");
    let mut unset = self::state(None);
    assert!(unset.listing(start) && unset.listed.text == r#"{"album":[]}"#);
    assert_eq!(unset.listing_wake(start + EVERY), Wake::Idle, "no folder chosen, nothing to look at");
}

#[test]
fn videos_are_listed_as_such_and_shown_ones_become_clips_of_the_scene() {
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(dir.path().join("a.png")).unwrap();
    std::fs::write(dir.path().join("waves.webm"), b"not decoded here").unwrap();
    let (mut state, start) = (state(Some(dir.path())), Instant::now());
    assert!(listed(&mut state, start));
    assert!(
        state.listed.text.contains(r#""waves.webm","video"]"#)
            && state.listed.text.contains(r#""a.png","image"]"#)
    );
    let key =
        state.listed.files.iter().find(|(_, p)| p.ends_with("waves.webm")).map(|(k, _)| k.clone()).unwrap();
    let mut outputs = ScriptOutputs::default();
    outputs
        .layers
        .insert("photo".into(), wprs_scene::LayerOverride { file: Some(key.clone()), ..Default::default() });
    state.show(&outputs);
    assert!(state.listed.reels.len() == 1 && state.listed.reels[0].0 == key);
    assert!(state.spec.clips.iter().any(|(k, reel)| *k == key && reel.path.ends_with("waves.webm")));
    assert!(state.spec.reels().any(|(k, ..)| k == key), "the compositor opens a player for it");
    state.show(&ScriptOutputs::default());
    assert!(state.listed.reels.is_empty() && !state.spec.clips.iter().any(|(k, _)| *k == key));
}

#[test]
fn a_watched_folder_is_looked_at_again_only_when_it_changes() {
    let dir = tempfile::tempdir().unwrap();
    image::RgbaImage::new(2, 2).save(dir.path().join("a.png")).unwrap();
    let mut state = state(Some(dir.path()));
    let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = Arc::clone(&woken);
    state.waker(|| {
        Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        })
    });
    let start = Instant::now();
    assert!(listed(&mut state, start), "the first listing never arrived");
    if state.listed.watch.is_none() {
        return eprintln!("no change notices on this system: the folder is looked at every 2 s instead");
    }
    let later = start + Duration::from_secs(600);
    assert_eq!(state.listing_wake(later), Wake::Idle, "an unchanged folder wakes nothing");
    assert!(!state.listing(later), "an unchanged folder was listed again");
    let before = woken.load(Ordering::SeqCst);
    image::RgbaImage::new(2, 2).save(dir.path().join("b.png")).unwrap();
    let told = (0..2000).any(|_| {
        state.listing_wake(later) == Wake::At(0.0) || (std::thread::sleep(Duration::from_millis(5)), false).1
    });
    assert!(told && woken.load(Ordering::SeqCst) > before, "no change notice arrived");
    assert!(listed(&mut state, later) && state.listed.text.contains("b.png"));
    assert_eq!(state.listing_wake(later), Wake::Idle);
}
