use super::*;

fn roped(render: Json, rate: f32) -> Json {
    with(
        sparks(),
        json!({ "emitters": [{ "shape": { "point": [20, 50] }, "rate": rate, "max_per_step": 1 }],
            "init": { "life": [2, 2], "speed": [60, 60], "angle": [0, 0], "size": [4, 4] },
            "forces": [{ "gravity": [0, 20] }], "collide": [], "render": render }),
    )
}

fn strung_after(system: Json, frames: usize, quality: Quality) -> (Cloud, Sim) {
    let mut rt = runtime(&scene(system, true));
    let bake = Bake::new(&rt.scene, 0, Tier::T3, true, |_| None).unwrap();
    let (mut cloud, mut last, mut stamp) = (Cloud::new(bake.allocs[0]), Sim::default(), 0);
    for _ in 0..frames {
        rt.advance(1.0 / 30.0);
        rt.frame(VIEW, quality);
        let swarm = rt.swarm(0).unwrap();
        if std::mem::replace(&mut stamp, swarm.stamp) != swarm.stamp {
            for s in &swarm.steps {
                last = bake.apply(0, s);
                cloud.step(&last, &bake.points);
            }
        }
    }
    cloud.strand(&last);
    (cloud, last)
}

#[test]
fn ropes_join_particles_newest_first_through_their_subdivisions() {
    let (cloud, sim) =
        strung_after(roped(json!({ "rope": { "subdivide": 3, "width": 6 } }), 30.0), 20, Quality::Q3);
    let alive = cloud.particles.iter().filter(|p| p.age < p.life).count();
    let drawn: Vec<&Strand> = cloud.rope.iter().take_while(|s| s[7] > 0.5).collect();
    assert_eq!(drawn.len(), (alive - 1) * 4 + 1, "{alive}");
    assert!(cloud.rope[drawn.len()..].iter().all(|s| *s == [0.0; 12]));
    assert!(drawn.windows(2).all(|w| w[1][0] > w[0][0] - 1e-3), "the newest particle starts the rope");
    assert!(drawn.iter().enumerate().all(|(q, s)| (s[4] - q as f32 / 4.0).abs() < 1e-4));
    assert!(drawn.iter().all(|s| (s[2].hypot(s[3]) - 1.0).abs() < 1e-3));
    assert_eq!(sim.rope[2], 3 | 1 << 8);
    let (_, low) =
        strung_after(roped(json!({ "rope": { "subdivide": 3, "width": 6 } }), 30.0), 4, Quality::Q1);
    assert_eq!(low.rope[2] & 255, 1);
    assert_eq!([Tier::T0, Tier::T1, Tier::T3].map(Tier::rope), [1024, 2048, MAX_ROPE]);
}

#[test]
fn ropes_never_draw_more_than_their_segments() {
    let mut sim = Sim { rope: [64, 0, 7 | 1 << 8, 40], ..Sim::default() };
    sim.ids[1] = 64;
    let particles: Vec<Particle> = (0..64)
        .map(|i| Particle {
            pos: [i as f32, 0.0],
            life: 9.0,
            id: i,
            look: [1.0; 2],
            color: [1.0; 4],
            ..Particle::default()
        })
        .collect();
    let (mut order, mut out) = (Vec::new(), Vec::new());
    strung(&particles, &sim, &mut order, &mut out);
    assert_eq!(out.len(), ROPE_POINTS);
    assert_eq!(out.iter().filter(|s| s[7] > 0.5).count(), 41);
    assert_eq!(out[0][0], 63.0);
    assert!((out[40][0] - 58.0).abs() < 1e-4);
}

#[test]
fn long_trails_keep_spaced_points_within_the_caps() {
    let long = Trail {
        length: 64,
        width: 0.5,
        segments: Some(15),
        subdivide: Some(2),
        uv_scale: None,
        uv_scroll: None,
        textured: false,
    };
    assert_eq!(long.kept(), Kept { length: 64, kept: 16, stride: 5, long: true });
    assert_eq!(long.kept().capped(Tier::T3, u32::MAX).kept, 16);
    let plain = Trail { length: 64, segments: None, subdivide: None, ..long.clone() };
    assert_eq!(plain.kept().capped(Tier::T3, u32::MAX), Kept { length: 64, kept: 64, stride: 1, long: true });
    assert_eq!(plain.kept().capped(Tier::T1, u32::MAX), Kept { length: 64, kept: 16, stride: 5, long: true });
    assert_eq!(plain.kept().capped(Tier::T0, u32::MAX).kept, 0);
    let short = Trail { length: 12, ..plain.clone() };
    assert_eq!(short.kept(), Kept { length: 12, kept: 12, stride: 1, long: false });
    assert_eq!(
        [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| q.long_trail(64)),
        [0, 16, 32, 64]
    );
    let render = json!({ "trail": { "length": 40, "width": 0.5, "segments": 8 } });
    let (cloud, sim) = strung_after(roped(render, 30.0), 40, Quality::Q3);
    assert_eq!(sim.trail[1..], [9, 5, 1]);
    let first = cloud.particles.iter().position(|p| p.age > 1.0 && p.age < p.life).unwrap();
    let marks = &cloud.trail[first * 9..first * 9 + 9];
    let x = |m: &[u32; 4]| f32::from_bits(m[0]);
    let gaps: Vec<f32> = marks.windows(2).map(|w| x(&w[0]) - x(&w[1])).collect();
    assert!(gaps[1..gaps.len() - 1].iter().all(|g| (g - 5.0).abs() < 1e-2), "{gaps:?}");
}

#[test]
fn ropes_flocks_and_walls_settle_once_their_particles_are_gone() {
    let mut s =
        roped(json!({ "rope": { "width": 4 }, "trail": { "length": 40, "width": 1, "subdivide": 2 } }), 0.0);
    s["emitters"][0] = json!({ "shape": { "point": [20, 50] }, "burst": { "count": 30 } });
    s["init"]["life"] = json!([0.3, 0.3]);
    s["forces"] = json!([{ "boids": { "radius": 20, "separation": 1, "alignment": 1, "cohesion": 1, "max_speed": 80 } }]);
    s["collide"] =
        json!([{ "bounds": { "box": { "from": [0, 0], "to": [200, 100], "inside": true } }, "stick": true }]);
    let mut rt = runtime(&scene(s, true));
    let mut wake = Wake::Frame;
    for _ in 0..30 {
        rt.advance(1.0 / 30.0);
        wake = rt.frame(VIEW, Quality::Q3).wake;
    }
    assert_eq!(wake, Wake::Idle);
}
