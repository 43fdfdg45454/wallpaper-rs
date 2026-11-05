use super::super::output::{Shown, Sink};
use super::super::scene::SceneState;
use super::super::{Content, SceneSpec};
use super::*;
use crate::scenes::ScenePack;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_scene::{Scene, Tier, Tonemap};

fn shown(scene: Option<Scene>) -> Shown {
    let power = Power::new(Tier::T3, false, 0);
    let state = scene.map(|scene| {
        let pack = Arc::new(ScenePack::bare(scene, Default::default(), Vec::new()));
        let spec = SceneSpec {
            values: pack.props.values(&[]),
            pack,
            extra: Vec::new(),
            clips: Vec::new(),
            hold: None,
            moving: false,
            quality: None,
            span: None,
        };
        SceneState::new(spec, power)
    });
    let content = Content::Color;
    let (fit, background, image, transition, uv, relief) = (Fit::Stretch, [0; 3], None, None, None, None);
    Shown { content, fit, background, image, transition, uv, relief, scene: state, tone: Default::default() }
}

#[test]
fn auto_hdr_waits_for_a_scene_that_tone_maps_at_the_top_quality() {
    let plain = Scene::from_json(include_str!("../../../../../scenes/snowfall/scene.json")).unwrap();
    let mut bright = plain.clone();
    (bright.canvas.hdr, bright.canvas.tonemap) = (true, Some(Tonemap::Agx));
    let mut flat = bright.clone();
    flat.canvas.tonemap = Some(Tonemap::None);
    let output = |scene| Output::new((16, 9), shown(scene), Sink::Stream(None), 60);
    assert!(!output(None).top(), "pictures, SDR videos and pages never ask for HDR");
    let mut film = output(None);
    film.bright_video = true;
    assert!(film.top(), "an HDR video asks for HDR like a bright scene");
    assert!(!output(Some(plain)).top());
    assert!(!output(Some(flat)).top());
    let lit = output(Some(bright));
    let q3 = lit.shown.scene.as_ref().unwrap().quality().quality == "q3";
    assert_eq!(lit.top(), q3);
    assert_eq!(HdrChoice::Auto.decide((true, false), false, true), (false, "display"));
    assert_eq!(
        HdrChoice::Auto.decide((true, true), false, lit.top()),
        (q3, if q3 { "auto" } else { "quality" })
    );
}

#[test]
fn an_hdr_video_takes_the_lowest_headroom_of_the_screens_that_show_it() {
    let reel = Reel { path: "/films/sea.webm".into(), cut: Default::default(), once: false };
    let movie = super::super::Movie { reel: reel.clone(), volume: 0, zoom: 1.0, pan: [0.0; 2], max_fps: 30 };
    let mut outputs = std::collections::BTreeMap::new();
    assert_eq!(video(&outputs, &reel), 1.0, "a video nobody shows is mapped to standard colors");
    let mut bright = Output::new((16, 9), shown(None), Sink::Stream(None), 60);
    bright.shown.content = Content::Video(movie.clone());
    bright.hdr = Hdr { active: true, peak_nits: Some(812.0), reference_nits: Some(203.0), ..Hdr::default() };
    outputs.insert("DP-1".to_string(), bright);
    assert!((video(&outputs, &reel) - 4.0).abs() < 1e-3);
    let mut dim = Output::new((16, 9), shown(None), Sink::Stream(None), 60);
    dim.shown.content = Content::Video(movie);
    outputs.insert("DP-2".to_string(), dim);
    assert_eq!(video(&outputs, &reel), 1.0, "a screen in standard range keeps the video standard");
}
