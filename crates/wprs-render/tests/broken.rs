mod hostile;

use hostile::{entry, frame, renderer, scene, show, unchecked};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wprs_render::{drawn_effects, set_build_timeout};
use wprs_scene::keep_strikes;

#[test]
fn a_shader_that_breaks_the_device_is_blamed_and_nothing_panics() {
    let Some(r) = renderer() else { return };
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("broken");
    drop(std::fs::remove_dir_all(&dir));
    keep_strikes(&dir.join("stopped-effects"));
    set_build_timeout(Duration::from_secs(3));
    let blamed = Arc::new(Mutex::new(None));
    let seen = blamed.clone();
    r.on_device_lost(move |_| *seen.lock().unwrap() = Some(drawn_effects()));
    let mut s = scene(&[("huge", entry("return tap(in.uv);"))]);
    let huge = "var a: array<vec4<f32>, 262144>; let i = u32(in.position.x) + u32(in.position.y) * 4096u; \
        a[i] = tap(in.uv); return a[(i * 7u) % 262144u] + vec4<f32>(0.25);";
    let key = unchecked(&mut s, "huge", &entry(huge));
    let left = match show(r, s) {
        Ok(mut shown) => {
            (0..3).for_each(|_| drop(frame(r, &mut shown)));
            drop(r.finish());
            shown.view.left_out()
        }
        Err(e) => vec![e.to_string()],
    };
    let blamed = blamed.lock().unwrap().clone();
    match blamed {
        Some(drawn) => assert!(r.lost() && drawn.contains(&key), "{drawn:?}"),
        None => assert!(left.iter().any(|l| l.contains("to build")), "{left:?}"),
    }
}
