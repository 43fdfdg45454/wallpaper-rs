use image::{Rgba, RgbaImage};
use wprs_core::edit::{Edit, Geometry};
use wprs_depth::{Info, Layers, turned};
use wprs_render::Renderer;

fn picture() -> RgbaImage {
    RgbaImage::from_fn(24, 12, |x, y| Rgba([(x * 10) as u8, (y * 20) as u8, 90, 255]))
}

fn layers(image: &RgbaImage) -> Layers {
    let info = Info { format: 1, size: [24, 12], depths: vec![0.5], ..Info::default() };
    Layers { info, images: vec![image.clone()] }
}

#[test]
fn an_identity_geometry_keeps_the_layers_and_the_others_change_their_frame() {
    let image = picture();
    let same = turned(&layers(&image), &Geometry::default());
    assert_eq!((same.info.size, same.images[0].clone()), ([24, 12], image.clone()));
    let rotated = turned(&layers(&image), &Geometry { rotate: 90, ..Geometry::default() });
    assert_eq!((rotated.info.size, rotated.images[0].dimensions()), ([12, 24], (12, 24)));
    let cropped =
        turned(&layers(&image), &Geometry { crop: Some([0.5, 0.0, 0.5, 0.5]), ..Geometry::default() });
    assert_eq!(cropped.images[0].dimensions(), (12, 6));
    assert_eq!(cropped.images[0].get_pixel(0, 0)[0].abs_diff(image.get_pixel(12, 0)[0]), 0);
}

#[test]
fn a_turned_layer_matches_what_the_gpu_draws_for_the_same_geometry() {
    let renderer = match Renderer::new() {
        Ok(renderer) => renderer,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let image = picture();
    for geometry in [
        Geometry { rotate: 90, ..Geometry::default() },
        Geometry { rotate: 270, flip: [true, false], ..Geometry::default() },
        Geometry { crop: Some([0.25, 0.5, 0.5, 0.5]), flip: [false, true], ..Geometry::default() },
        Geometry { rotate: 180, crop: Some([0.0, 0.0, 0.5, 1.0]), ..Geometry::default() },
    ] {
        let edit = Edit { geometry: geometry.clone(), ..Edit::default() };
        let drawn = renderer.export_edit(&image, &edit, None, None).unwrap();
        let mine = turned(&layers(&image), &geometry).images.remove(0);
        assert_eq!(drawn.dimensions(), mine.dimensions(), "{geometry:?}");
        let far = drawn
            .pixels()
            .zip(mine.pixels())
            .filter(|(a, b)| a.0[..3].iter().zip(&b.0).any(|(x, y)| x.abs_diff(*y) > 12))
            .count();
        assert!(far * 20 < drawn.pixels().len(), "{geometry:?}: {far} pixels differ");
    }
}
