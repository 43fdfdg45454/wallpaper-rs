use super::*;

#[test]
fn scroll_wraps_around_the_layer() {
    let Some(mut run) = Fx::new("scroll", json!({ "speed": [0.5, 0] })).input(halves(RED, BLUE)).run() else {
        return;
    };
    run.shot().check(&[((10, 16), RED), ((54, 16), BLUE)]);
    run.runtime.advance(1.0);
    run.shot().check(&[((10, 16), BLUE), ((54, 16), RED), ((0, 16), BLUE), ((63, 16), RED)]);
    run.runtime.advance(1.0);
    run.shot().check(&[((10, 16), RED), ((54, 16), BLUE)]);
}
