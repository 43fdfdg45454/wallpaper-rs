use super::*;

fn flow(capacity: u32, rate: f32, speed: f32) -> Value {
    json!({ "flow": { "capacity": capacity, "seed": 3,
        "emitters": [{ "shape": { "point": [20, 20] }, "rate": rate }],
        "init": { "life": [30, 30], "speed": [speed, speed], "size": [1, 1], "color": ["#ffffff", "#ffffff"] } } })
}

fn alive(r: &Renderer, at: [f32; 2]) -> Option<u32> {
    let layer =
        json!([{ "id": "f", "type": "particles", "system": "flow", "transform": { "position": at } }]);
    let mut run = Run::new(r, flow(4000, 300.0, 0.0), layer, true)?;
    (0..20).for_each(|_| drop(run.frame(r, 1.0 / 30.0)));
    run.census(r).first().map(|c| c.alive)
}

#[test]
fn particles_off_the_screen_keep_simulating_while_nothing_on_it_changes() {
    let Some(r) = renderer() else { return };
    let (Some(shown), Some(away)) = (alive(r, [0.0, 0.0]), alive(r, [5000.0, 5000.0])) else { return };
    assert!(shown > 100, "{shown}");
    assert_eq!(away, shown, "steps of a particle layer off the screen were lost");
}

fn gpu_ms(r: &Renderer, dt: f64) -> Option<[f64; 2]> {
    let mut systems = flow(200_000, 0.0, 40.0);
    systems["flow"]["emitters"][0]["burst"] = json!({ "count": 200_000 });
    let layer = json!([{ "id": "f", "type": "particles", "system": "flow" }]);
    let mut run = Run::new(r, systems, layer, true)?;
    run.frame(r, 1.0 / 60.0);
    let (mut seen, mut times) = (run.view.gpu_time().0, Vec::new());
    for _ in 0..30 {
        run.frame(r, dt);
        let (samples, stats) = (run.view.gpu_time().0, run.view.stats());
        let timed = [stats.sim_gpu_us, stats.gpu_us].map(|us| f64::from(us) / 1000.0);
        times.extend((std::mem::replace(&mut seen, samples) != samples).then_some(timed));
    }
    times.sort_by(|a, b| a[0].total_cmp(&b[0]));
    times.get(times.len() / 2).copied()
}

#[test]
fn the_gpu_time_of_a_frame_counts_its_particle_simulation() {
    let Some(r) = renderer() else { return };
    let (Some(one), Some(many)) = (gpu_ms(r, 1.0 / 60.0), gpu_ms(r, 0.25)) else { return };
    assert!(one[0] > 0.0 && one[1] >= one[0] && many[1] >= many[0], "{one:?} {many:?}");
    assert!(many[0] > one[0] * 2.0, "{one:?} ms with one step a frame, {many:?} with fifteen");
}
