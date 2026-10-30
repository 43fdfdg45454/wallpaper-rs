use super::*;

fn moved(rt: &mut Runtime, at: [f32; 2]) {
    let set = LayerOverride { position: Some(at), ..LayerOverride::default() };
    rt.set_script(ScriptOutputs { layers: [("p".to_string(), set)].into(), ..ScriptOutputs::default() });
}

#[test]
fn world_space_particles_stay_where_they_were_born() {
    let burst = json!([{ "shape": { "point": [100, 50] }, "burst": { "count": 1, "every": 0.5 } }]);
    for (space, second) in [("world", [60.0, 30.0]), ("local", [100.0, 50.0])] {
        let system = still(json!({ "space": space, "emitters": burst }));
        let layers = json!([{ "id": "p", "type": "particles", "system": "a", "transform": { "position": [150, 50] } }]);
        let mut fly = Fly::new(runtime(json!({ "a": system }), layers));
        fly.frame(0.0);
        let first = if space == "world" { [150.0, 50.0] } else { [100.0, 50.0] };
        assert!(near(fly.alive(0)[0].pos, first, 0.01), "{space}: {:?}", fly.alive(0)[0].pos);
        moved(&mut fly.rt, [60.0, 30.0]);
        fly.run(0.5);
        let mut at: Vec<[f32; 2]> = fly.alive(0).iter().map(|p| p.pos).collect();
        at.sort_by(|a, b| b[0].total_cmp(&a[0]));
        assert!(at.len() == 2 && near(at[0], first, 0.01), "{space}: {at:?}");
        assert!(at.iter().any(|p| near(*p, second, 0.01)), "{space}: {at:?}");
        assert_eq!(fly.rt.flocks()[0].layer.is_some(), space == "world");
    }
}

#[test]
fn control_points_place_emitters_and_follow_instances() {
    let points = json!([{ "at": [0, 0] }, { "at": [40, 20], "angle": 90 }]);
    let shape = json!({ "box": { "from": [0, 0], "to": [10, 2], "at": "p1" } });
    let system =
        still(json!({ "points": points, "emitters": [{ "shape": shape, "burst": { "count": 60 } }] }));
    let mut fly = Fly::of(json!({ "a": system }));
    fly.frame(0.0);
    let born = fly.alive(0);
    assert_eq!(born.len(), 60);
    assert!(
        born.iter().all(|p| (37.9..=40.1).contains(&p.pos[0]) && (19.9..=30.1).contains(&p.pos[1])),
        "{born:?}"
    );
    let moving = json!([{ "id": "p", "type": "particles", "system": "a", "instance": { "points": [[0, 0], [150, 80]] } }]);
    let mut fly = Fly::new(runtime(json!({ "a": system }), moving));
    fly.frame(0.0);
    assert!(fly.alive(0).iter().all(|p| (147.9..=150.1).contains(&p.pos[0]) && p.pos[1] >= 79.9));
    let pulled = still(
        json!({ "points": points, "forces": [{ "attractor": { "target": "p1", "strength": 4000, "falloff": "constant" } }] }),
    );
    let mut fly = Fly::of(json!({ "a": pulled }));
    fly.run(0.2);
    assert!(fly.alive(0).iter().all(|p| p.pos[0] < 100.0 && p.pos[1] < 50.0), "pulled toward p1");
}

#[test]
fn instances_scale_births_time_and_looks() {
    let flowing =
        still(json!({ "capacity": 1000, "emitters": [{ "shape": { "point": [100, 50] }, "rate": 600 }] }));
    let tuned = json!({ "count": 0.5, "time_scale": 2, "life": 2, "alpha": 0.5, "size": 3, "brightness": 2, "color": "#ff0000" });
    let layers = json!([{ "id": "p", "type": "particles", "system": "a", "instance": tuned }]);
    let mut fly = Fly::new(runtime(json!({ "a": flowing }), layers));
    fly.run(1.0);
    let step = *fly.rt.swarm(0).unwrap().sim(0, 0);
    assert_eq!((step.ids[3], step.init[0][1]), (500, 20.0));
    let births = fly.births;
    assert!((550..=650).contains(&births), "half the rate at twice the speed: {births}");
    let frame = fly.rt.frame(VIEW, Quality::Q3);
    let d = &frame.draws[0];
    assert!(matches!(d.content, Content::Particles { size, .. } if size == 3.0));
    assert_eq!((d.opacity, d.brightness, d.tint), (0.5, 2.0, Color([1.0, 0.0, 0.0, 1.0])));
    let shared = json!([{ "id": "p", "type": "particles", "system": "a", "instance": { "alpha": 0.5 } },
        { "id": "q", "type": "particles", "system": "a" }]);
    assert_eq!(runtime(json!({ "a": flowing }), shared).flocks().len(), 1, "looks alone share the particles");
}

#[test]
fn scripts_pause_emit_stop_and_play_particles() {
    let flowing = still(json!({ "emitters": [{ "shape": { "point": [100, 50] }, "rate": 60 }] }));
    let mut fly = Fly::of(json!({ "a": flowing }));
    fly.run(0.5);
    let say = |fly: &mut Fly, commands: Vec<ParticleCommand>| {
        let control = ParticleControl { commands, ..ParticleControl::default() };
        fly.rt.set_script(ScriptOutputs {
            particles: [("p".to_string(), control)].into(),
            ..ScriptOutputs::default()
        });
    };
    say(&mut fly, vec![ParticleCommand::Pause]);
    let before = fly.births;
    fly.run(0.5);
    assert_eq!(fly.births, before, "a paused system emits nothing");
    say(&mut fly, vec![ParticleCommand::Emit(7)]);
    fly.run(0.1);
    assert_eq!(fly.births, before + 7);
    say(&mut fly, vec![ParticleCommand::Stop]);
    fly.frame(1.0 / 30.0);
    assert!(fly.alive(0).is_empty() && fly.rt.swarm(0).unwrap().play == Play::Stopped);
    say(&mut fly, vec![ParticleCommand::Play]);
    fly.run(0.5);
    assert!(fly.alive(0).len() >= 25, "{}", fly.alive(0).len());
}
