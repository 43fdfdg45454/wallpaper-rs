mod hostile;

use hostile::{entry, frame, renderer, scene, show, unchecked};
use wprs_render::Renderer;
use wprs_scene::Quality;

#[test]
fn a_lost_device_returns_errors_instead_of_panicking() {
    if renderer().is_none() {
        return;
    }
    let r = Renderer::new().unwrap();
    let mut shown = show(&r, scene(&[("e", entry("return tap(in.uv);"))])).unwrap();
    let now = shown.runtime.frame(shown.view.view((64, 32)), Quality::Q3);
    let drawn = r.render_scene(&mut shown.view, &shown.runtime, &now, (64, 32), Quality::Q3, &[]);
    let image = drawn.unwrap().clone();
    r.finish().unwrap();
    r.destroy_device();
    assert!(r.finish().is_err());
    assert!(r.finish().is_err());
    drop(frame(&r, &mut shown));
    assert!(r.finish().is_err() && r.read_image(&image).is_err() && r.lost());
    let _ = unchecked;
}
