use super::*;

fn phased(base: &Sim, t: f64) -> Sim {
    let mut s = *base;
    s.clock[1] = t as f32;
    s.phases(t);
    s
}

fn sweeping() -> Sim {
    let mut s = Sim::default();
    s.counts[1] = 2;
    (s.shapes[0], s.emitters[1]) = (6, [12.0, 0.0, 0.0, 30.0]);
    (s.shapes[1], s.emitters[5], s.paths[3]) = (7, [8.0, 1.0, 0.0, 1.0], [0.5, 0.0, 0.0, 0.0]);
    (s.fields[0], s.fields[1]) = ([0.0, 1.0, 2.0, 0.0], [10.0, 40.0, 0.0, 1.0]);
    (s.fields[2], s.fields[3]) = ([10.0, 1.0, 0.0, 1.5], [0.0, 0.0, 2.0, 1.0]);
    s.remap[0] = 1;
    (s.remaps[0], s.remaps[2]) = ([1.0, 0.0, 10.0, 0.0], [1.0, 0.0, 0.0, 0.0]);
    s
}

#[test]
fn time_phases_move_by_one_step_even_after_days_of_uptime() {
    let base = sweeping();
    let dt = 1.0 / 60.0;
    let moved = |a: f32, b: f32, period: f32| (b - a).rem_euclid(period);
    for t in [2.0, 5.2e5 + 0.37, 1.0e6, 1.0e7] {
        let (a, b) = (phased(&base, t), phased(&base, t + dt));
        let turned = moved(a.emitters[1][3], b.emitters[1][3], 360.0);
        assert!((turned - 0.5).abs() < 1e-3, "at {t} s the ring turned {turned}°");
        let slid = moved(a.paths[3][0], b.paths[3][0], 1.0);
        assert!((slid - 0.5 / 60.0).abs() < 1e-4, "at {t} s the taper moved {slid}");
        let gust = moved(a.fields[0][2], b.fields[0][2], 4096.0);
        assert!((gust - 2.0 / 60.0).abs() < 1e-3, "at {t} s the turbulent noise moved {gust}");
        let drift = moved(a.fields[2][3], b.fields[2][3], 4096.0);
        assert!((drift - 1.5 / 60.0).abs() < 1e-3, "at {t} s the offset drifted {drift}");
        let run = moved(a.remaps[0][3], b.remaps[0][3], 10.0);
        assert!((run - dt as f32).abs() < 1e-4, "at {t} s the runtime moved {run}");
    }
    let early = phased(&base, 2.0);
    assert_eq!((early.emitters[1][3], early.fields[0][2], early.remaps[0][3]), (60.0, 4.0, 2.0));
}

#[test]
fn births_around_a_point_turn_smoothly_after_days_of_uptime() {
    let ring = json!({ "around": { "at": [100, 50], "count": 1, "radius": 20, "speed": 30 } });
    let system = with(
        sparks(),
        json!({ "emitters": [{ "shape": ring, "rate": 60 }], "forces": [], "collide": [],
        "init": { "life": [5, 5], "speed": [0, 0], "size": [2, 2] } }),
    );
    let mut rt = runtime(&scene(system, true));
    let bake = Bake::new(&rt.scene, 0, Tier::T3, false, |_| None).unwrap();
    rt.advance(1.0 / 30.0);
    rt.frame(VIEW, Quality::Q3);
    let mut base = *rt.swarm(0).unwrap().sim(0, 0);
    base.emitters[1][3] = 30.0;
    let born = |t: f64| {
        let mut cloud = Cloud::new(bake.allocs[0]);
        cloud.step(&bake.apply(0, &phased(&base, t)), &bake.points);
        let p = cloud.particles.iter().find(|p| p.age < p.life).copied().unwrap();
        (p.pos[1] - 50.0).atan2(p.pos[0] - 100.0).to_degrees()
    };
    for t in [1.0e6, 1.0e6 + 4.0 / 60.0] {
        let (a, b) = (born(t), born(t + 1.0 / 60.0));
        let turned = (b - a).rem_euclid(360.0);
        assert!((turned - 0.5).abs() < 0.01, "at {t} s births turned {turned}°");
    }
}
