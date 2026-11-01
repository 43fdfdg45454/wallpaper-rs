use super::super::output::{Shown, Sink};
use super::*;
use wprs_core::{Fit, Place};

#[test]
fn one_developed_picture_serves_every_screen_that_needs_about_its_size() {
    assert!(serves((2000, 1000), (2000, 1000)) && serves((1990, 995), (2000, 1000)));
    assert!(serves((3000, 1500), (2000, 1000)) && !serves((4100, 2050), (2000, 1000)));
    assert!(!serves((1900, 950), (2000, 1000)));
}

#[test]
fn screens_that_show_the_same_picture_share_one_texture() {
    let renderer = match wprs_render::Renderer::shared() {
        Ok(renderer) => renderer,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let texture = |w: u32, h: u32| renderer.upload_rgba(&vec![0; (w * h * 4) as usize], w, h, w * 4).unwrap();
    let (file, place) = (PathBuf::from("/pictures/hills.jpg"), Place::Fit);
    let picture = Picture { file, modified: None, pixels: None, edit: None, place, depth: None };
    let output = |image: GpuImage| {
        let (content, fit, background) = (Content::Image(picture.clone()), Fit::Fill, [0; 3]);
        let (transition, uv, relief, scene) = (None, None, None, None);
        let shown = Shown {
            content,
            fit,
            background,
            image: Some(image),
            transition,
            uv,
            relief,
            scene,
            tone: Default::default(),
        };
        Output::new((16, 9), shown, Sink::Stream(None), 60)
    };
    let mut outputs =
        BTreeMap::from([("A".into(), output(texture(1000, 500))), ("B".into(), output(texture(64, 32)))]);
    let mut pictures = Pictures::default();
    let made = Developed { look: look(&picture), image: texture(1200, 600), relief: None };
    pictures.keep(made.clone(), &mut outputs);
    assert!(
        outputs["A"].shown.image.as_ref().is_some_and(|i| i.same(&made.image)),
        "the first screen kept its copy"
    );
    assert!(
        !outputs["B"].shown.image.as_ref().unwrap().same(&made.image),
        "a much smaller screen took a large copy"
    );
    assert!(pictures.find(&made.look, (1100, 550)).is_some_and(|d| d.image.same(&made.image)));
    pictures.prune(&outputs);
    assert_eq!(pictures.0.len(), 1);
    outputs.remove("A");
    pictures.prune(&outputs);
    assert!(pictures.0.is_empty(), "a texture nobody shows was kept");
}

#[test]
fn a_second_screen_finds_the_texture_made_from_a_reduced_decode() {
    let renderer = match wprs_render::Renderer::shared() {
        Ok(renderer) => renderer,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("wide.png");
    RgbaImage::from_pixel(300, 200, image::Rgba([0, 90, 0, 255])).save(&file).unwrap();
    let picture = Picture { file, modified: None, pixels: None, edit: None, place: Place::Fit, depth: None };
    let screen = (Fit::Fill, (192, 108));
    let made = develop(renderer, "A", &picture, None, screen).expect("a texture");
    assert_eq!(made.0.size(), (192, 128), "the picture was not decoded at the size the screen needs");
    let mut pictures = Pictures::default();
    pictures
        .keep(Developed { look: look(&picture), image: made.0.clone(), relief: None }, &mut BTreeMap::new());
    let found =
        pictures.shared(renderer, &picture, None, screen).expect("the second screen made its own copy");
    assert!(found.0.same(&made.0));
    let decoded = Some((192, 128));
    assert!(pictures.shared(renderer, &picture, decoded, screen).is_some_and(|f| f.0.same(&made.0)));
    assert!(pictures.shared(renderer, &picture, Some((300, 200)), (Fit::Fit, (600, 400))).is_none());
}
