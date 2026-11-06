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

#[test]
fn hdr_ktx2_pictures_ask_for_hdr_and_keep_their_light_while_the_screen_has_it() {
    use super::super::Picture;
    let dir = tempfile::tempdir().unwrap();
    let sun = image::Rgba32FImage::from_fn(16, 8, |x, _| {
        image::Rgba([if x < 8 { 6.0 } else { 0.5 }, 0.3, 0.2, 1.0])
    });
    let options = wprs_ktx2::Options {
        codec: wprs_ktx2::Codec::Uastc,
        quality: 90,
        srgb: true,
        mips: false,
        repeat: false,
    };
    let file = dir.path().join("sun.ktx2");
    std::fs::write(&file, wprs_ktx2::encode_light(&[sun], &options).unwrap()).unwrap();
    let picture = |file: &std::path::Path, edit: Option<Arc<wprs_core::edit::Edit>>| {
        let place = wprs_core::Place::Fit;
        Content::Image(Picture {
            file: file.to_owned(),
            modified: None,
            pixels: None,
            edit,
            place,
            depth: None,
        })
    };
    let mut output = Output::new((16, 9), shown(None), Sink::Stream(None), 60);
    output.shown.content = picture(&dir.path().join("plain.png"), None);
    super::super::pictures::follow(&mut output);
    assert!(!output.top(), "a standard picture asked for HDR");
    output.shown.content = picture(&file, None);
    super::super::pictures::follow(&mut output);
    assert!(output.top(), "an HDR picture did not ask for HDR");
    let mut turned = wprs_core::edit::Edit::default();
    turned.geometry.rotate = 90;
    output.shown.content = picture(&file, Some(Arc::new(turned)));
    super::super::pictures::follow(&mut output);
    assert!(!output.top(), "an edited picture, drawn in standard range, asked for HDR");
    output.shown.content = picture(&file, None);
    super::super::pictures::follow(&mut output);
    let Ok(renderer) = wprs_render::Renderer::shared() else {
        return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none());
    };
    assert!(!output.lit[0].relight(renderer, 1.0) && output.lit[0].image().is_none());
    assert!(output.lit[0].relight(renderer, 4.0), "a screen with HDR got no light");
    let lit = output.lit[0].image().unwrap();
    assert!(renderer.read_light(lit).unwrap().iter().any(|t| t[0] > 2.0), "the light stayed under white");
    let id = lit.id();
    assert!(!output.lit[0].relight(renderer, 4.0), "the same peak built the light again");
    let players = HashMap::new();
    let layer = output.shown.layer(&players, 1.0, None, (16, 9)).lit(output.lit[0].image());
    assert_eq!(layer.image.map(wprs_render::GpuImage::id), Some(id));
    output.show(shown(None), (16, 9), false);
    assert!(output.lit[0].image().is_none(), "the light outlived its picture");
}
