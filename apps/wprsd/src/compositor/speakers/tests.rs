use super::super::Content;
use super::super::output::{Shown, Sink};
use super::*;
use crate::compositor::SceneSpec;
use wprs_core::Fit;
use wprs_scene::{Scene, Tier};

#[test]
fn scenes_start_muted_play_on_one_output_and_never_while_locked() {
    let pond = PathBuf::from("/pond.wpak");
    let shown = |live: [bool; 2]| {
        vec![("A".into(), pond.clone(), live[0], 1.0), ("B".into(), pond.clone(), live[1], 0.5)]
    };
    let muted = crate::daemon::settings(&wprs_core::Config::default());
    assert!(choose(shown([true, true]), muted, false).is_empty());
    let heard = Settings { muted: false, ..muted };
    let one = Chosen::from([(pond.clone(), ("A".into(), true, 0.5))]);
    assert_eq!(choose(shown([true, true]), heard, false), one);
    assert_eq!(choose(shown([false, true]), heard, false)[&pond], ("B".into(), true, 0.25));
    assert_eq!(choose(shown([false, false]), heard, false)[&pond], ("A".into(), false, 0.5));
    assert_eq!(choose(shown([true, true]), heard, true)[&pond], ("A".into(), false, 0.5), "locked");
    assert!(choose(shown([true, true]), Settings { volume: 0.0, ..heard }, false).is_empty());
    assert!(
        choose(shown([true, true]), Settings { scene_sound: false, ..heard }, false).is_empty(),
        "blocked"
    );
    let quiet_first = vec![("A".into(), pond.clone(), true, 0.0), ("B".into(), pond.clone(), true, 0.5)];
    assert_eq!(choose(quiet_first, heard, false)[&pond], ("B".into(), true, 0.25), "A is turned down");
}

fn delayed() -> SceneState {
    let text = serde_json::json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [16, 9] },
        "assets": { "wind": { "path": "wind.ogg" } }, "timelines": [{ "id": "t", "duration": 9 }],
        "layers": [{ "id": "wind", "type": "sound", "sound": "wind",
                     "play": { "timeline": "t", "keys": [[0, false], [1.5, true]] } }] });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    let sounds = vec![("wind".into(), PathBuf::from("/wind.ogg"))];
    let pack = Arc::new(ScenePack::bare(scene, Default::default(), sounds));
    let spec =
        SceneSpec { values: Default::default(), pack, extra: Vec::new(), clips: Vec::new(), hold: None };
    SceneState::new(spec, super::super::power::Power::new(Tier::T3, false, 30))
}

#[test]
fn the_scene_clock_of_a_sound_stops_while_the_scene_pauses() {
    let mut scene = delayed();
    let (start, tick) = (Instant::now(), Duration::from_millis(100));
    let mut at = |seconds: f64| {
        scene.hear(tick, start + Duration::from_secs_f64(seconds));
        (scene.voices[0].play, scene.heard.map(|at| at - start))
    };
    assert_eq!([at(0.0), at(1.0)], [(false, Some(tick)), (false, Some(Duration::from_millis(1100)))]);
    scene.pause();
    let mut at = |seconds: f64| {
        scene.hear(tick, start + Duration::from_secs_f64(seconds));
        scene.voices[0].play
    };
    assert_eq!([at(5.0), at(5.4), at(5.6)], [false, false, true]);
}

#[test]
fn a_silent_scene_wakes_the_loop_on_its_clock_before_any_speaker_exists() {
    let (mut scene, now, tick) = (delayed(), Instant::now(), Duration::from_millis(100));
    scene.hear(tick, now);
    let (content, background) = (Content::Scene(scene.spec.clone()), [0; 3]);
    let (image, transition, uv, relief, scene) = (None, None, None, None, Some(scene));
    let shown = Shown { content, fit: Fit::Stretch, background, image, transition, uv, relief, scene };
    let outputs = BTreeMap::from([("A".into(), Output::new((16, 9), shown, Sink::Stream(None), 60))]);
    let chosen = |live| Chosen::from([(PathBuf::new(), ("A".into(), live, 1.0))]);
    assert_eq!(ticks(&outputs, chosen(true)).collect::<Vec<_>>(), [now + tick]);
    assert_eq!(ticks(&outputs, chosen(false)).count(), 0, "a paused scene sleeps");
}

#[test]
fn a_playing_speaker_whose_scene_clock_stands_still_wakes_once_a_second() {
    let (scene, start) = (delayed(), Instant::now());
    let (pack, voices) = (scene.spec.pack.clone(), scene.voices.clone());
    let mut speaker = Speaker {
        pack,
        output: "A".into(),
        track: None,
        playing: true,
        volume: 1.0,
        voices: voices.clone(),
        fade: Duration::ZERO,
        quiet: None,
        synced: start,
    };
    for second in 1..4 {
        assert_eq!(speaker.due(), Some(start + RESYNC * second));
        speaker.follow(&voices, start + RESYNC * second);
    }
}
