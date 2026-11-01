use super::super::output::Shown;
use super::super::output::Sink;
use super::super::scene::SceneState;
use super::super::{Content, SceneSpec};
use super::*;
use crate::scenes::ScenePack;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_scene::{Scene, Tier};

fn snowfall() -> SceneSpec {
    let scene = Scene::from_json(include_str!("../../../../../scenes/snowfall/scene.json")).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../scenes/snowfall/manifest.json")).unwrap();
    let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
    let pack = Arc::new(ScenePack::bare(scene, properties, Vec::new()));
    SceneSpec {
        values: pack.props.values(&[]),
        pack,
        extra: Vec::new(),
        clips: Vec::new(),
        hold: None,
        moving: false,
    }
}

#[test]
fn fades_never_outrun_a_known_display_or_the_battery_cap() {
    let color = || {
        let (content, fit, background) = (Content::Color, Fit::Stretch, [0; 3]);
        let (image, transition, uv, relief, scene) = (None, None, None, None, None);
        Shown { content, fit, background, image, transition, uv, relief, scene, tone: Default::default() }
    };
    let fade = |refresh, on_battery| {
        let mut output = Output::new((16, 9), color(), Sink::Stream(None), refresh);
        output.fade = Some((color(), Some(Instant::now())));
        interval(&output, Power::new(Tier::T3, on_battery, 0))
    };
    let every = |fps: u32| Duration::from_secs(1) / fps;
    let fades = [fade(20, false), fade(0, false), fade(144, false), fade(144, true)];
    assert_eq!(fades, [every(20), every(60), every(144), every(30)]);
}

#[test]
fn a_failed_draw_is_tried_again_later_and_less_often() {
    let color = || Shown {
        content: Content::Color,
        fit: Fit::Stretch,
        background: [0; 3],
        image: None,
        transition: None,
        uv: None,
        relief: None,
        scene: None,
        tone: Default::default(),
    };
    let (power, now) = (Power::new(Tier::T3, false, 0), Instant::now());
    let mut output = Output::new((16, 9), color(), Sink::Stream(None), 60);
    (output.failed, output.last_render) = (1, Some(now));
    assert_eq!(deadline(&output, true, power, now), Some(now + RETRY));
    output.failed = 3;
    assert_eq!(deadline(&output, true, power, now), Some(now + RETRY * 4));
    output.failed = 40;
    assert_eq!(deadline(&output, true, power, now), Some(now + RETRY_MAX));
    output.failed = 0;
    assert_eq!(deadline(&output, true, power, now), Some(now + Duration::from_secs(1) / 60));
    output.failed = 7;
    let red = Shown { background: [255, 0, 0], ..color() };
    output.show(red, (16, 9), false);
    assert_eq!(
        deadline(&output, true, power, now),
        Some(now + Duration::from_secs(1) / 60),
        "a new picture waited for the retries of the old one"
    );
    let Ok(renderer) = Renderer::shared() else { return };
    output.sink = Sink::Stream(Some(renderer.create_target(16, 9).unwrap()));
    (output.stale, output.dirty) = (false, false);
    output.unwatched(true);
    assert!(matches!(output.sink, Sink::Stream(Some(_))));
    output.unwatched(false);
    assert!(matches!(output.sink, Sink::Stream(None)), "a stream nobody watches kept its target");
}

#[test]
fn a_scene_that_follows_the_display_asks_for_120_fps_on_a_120_hz_stream() {
    let renderer = match Renderer::shared() {
        Ok(renderer) => renderer,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let cadence = |power: Power| {
        let (spec, now) = (snowfall(), Instant::now());
        let scene = Some(SceneState::new(spec.clone(), power));
        let (content, image, transition, uv, relief) = (Content::Scene(spec), None, None, None, None);
        let shown = Shown {
            content,
            fit: Fit::Stretch,
            background: [0; 3],
            image,
            transition,
            uv,
            relief,
            scene,
            tone: Default::default(),
        };
        let mut output = Output::new((16, 9), shown, Sink::Stream(None), 120);
        let scene = output.shown.scene.as_mut().expect("a scene");
        scene.render(renderer, (output.size, output.refresh), power, &HashMap::new(), true, now).unwrap();
        let due = scene.due.map(|at| at - now);
        output.last_render = Some(now);
        (due, deadline(&output, true, power, now).map(|at| at - now))
    };
    let every = |fps: u32| Some(Duration::from_secs(1) / fps);
    let mains = Power::new(Tier::T3, false, 0);
    assert_eq!(cadence(mains), (every(120), every(120)));
    assert_eq!(cadence(Power { on_battery: true, ..mains }), (every(30), every(30)));
    assert_eq!(cadence(Power { max_fps: 60, ..mains }), (every(60), every(60)));
}

#[test]
fn a_scene_frame_that_changed_nothing_is_not_composed_again() {
    let renderer = match Renderer::shared() {
        Ok(renderer) => renderer,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let (power, now) = (Power::new(Tier::T3, false, 60), Instant::now());
    let text = r##"{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [32, 18] },
      "timelines": [{ "id": "t", "duration": 4, "loop": "repeat" }],
      "layers": [{ "id": "card", "type": "solid", "color": "#4060ff", "size": [16, 9],
        "transform": { "rotation": { "timeline": "t", "keys": [[0, 0], [4, 360]] } } }] }"##;
    let pack = Arc::new(ScenePack::bare(Scene::from_json(text).unwrap(), Default::default(), Vec::new()));
    let spec = SceneSpec {
        values: Default::default(),
        pack,
        extra: Vec::new(),
        clips: Vec::new(),
        hold: None,
        moving: false,
    };
    let (content, fit, background) = (Content::Scene(spec.clone()), Fit::Stretch, [0; 3]);
    let (image, transition, uv, relief, scene) = (None, None, None, None, Some(SceneState::new(spec, power)));
    let shown =
        Shown { content, fit, background, image, transition, uv, relief, scene, tone: Default::default() };
    let mut output = Output::new((16, 9), shown, Sink::Stream(None), 60);
    let frame = |output: &mut Output, starved: bool, at: Instant| {
        let scene = output.shown.scene.as_mut().expect("a scene");
        scene.render(renderer, (output.size, 60), power, &HashMap::new(), true, at).unwrap();
        output.unchanged(None, starved)
    };
    let later = now + Duration::from_millis(100);
    assert!(!frame(&mut output, false, now), "the first frame of a scene was not composed");
    output.stale = false;
    assert!(frame(&mut output, false, now), "a frame that changed nothing was composed again");
    assert!(!frame(&mut output, false, later), "a moving frame was skipped");
    assert!(!frame(&mut output, true, later), "a starved stream got no frame");
    output.composed = None;
    assert!(!frame(&mut output, false, later), "a repaint was skipped");
    assert!(frame(&mut output, false, later));
    let (image, transition, uv, relief, scene) = (None, None, None, None, None);
    let color = Shown {
        content: Content::Color,
        fit,
        background,
        image,
        transition,
        uv,
        relief,
        scene,
        tone: Default::default(),
    };
    output.fade = Some((color, None));
    assert!(!frame(&mut output, false, later), "a fade was skipped");
}
