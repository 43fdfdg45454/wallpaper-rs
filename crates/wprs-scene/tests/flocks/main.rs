use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

mod births;
mod drives;
mod kin;
mod looks;
mod spaces;
mod timing;

const VIEW: [f32; 4] = [0.0, 0.0, 200.0, 100.0];

fn runtime(systems: Json, layers: Json) -> Runtime {
    let text =
        json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": layers, "particles": systems });
    let scene = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
    Runtime::new(Arc::new(scene), Values::new())
}

fn layer(system: &str) -> Json {
    json!([{ "id": "p", "type": "particles", "system": system }])
}

fn still(more: Json) -> Json {
    let mut s = json!({ "capacity": 400, "seed": 5, "emitters": [{ "shape": { "point": [100, 50] }, "burst": { "count": 50 } }],
        "init": { "life": [10, 10], "speed": [0, 0], "size": [2, 2] } });
    more.as_object().unwrap().iter().for_each(|(k, v)| s[k] = v.clone());
    s
}

struct Fly {
    rt: Runtime,
    bake: Bake,
    clouds: Vec<Cloud>,
    stamp: u64,
    epoch: u32,
    births: u32,
}

impl Fly {
    fn new(rt: Runtime) -> Fly {
        Fly::with(rt, |_| None)
    }

    fn with<'a>(rt: Runtime, image: impl Fn(&str) -> Option<(u32, u32, &'a [u8])>) -> Fly {
        let bake = Bake::new(&rt.scene, 0, Tier::T3, false, image).unwrap();
        let clouds = bake.allocs.iter().map(|a| Cloud::new(*a)).collect();
        Fly { rt, bake, clouds, stamp: 0, epoch: 0, births: 0 }
    }

    fn of(systems: Json) -> Fly {
        Fly::new(runtime(systems, layer("a")))
    }

    fn frame(&mut self, dt: f64) -> Wake {
        self.rt.advance(dt);
        let wake = self.rt.frame(VIEW, Quality::Q3).wake;
        let swarm = self.rt.swarm(0).unwrap();
        if swarm.epoch != self.epoch {
            self.clouds.iter_mut().for_each(Cloud::clear);
        }
        self.epoch = swarm.epoch;
        if std::mem::replace(&mut self.stamp, swarm.stamp) != swarm.stamp {
            for j in 0..swarm.count() {
                let sims: Vec<Sim> = (0..swarm.parts).map(|p| self.bake.apply(p, swarm.sim(j, p))).collect();
                self.births += sims[0].total();
                Cloud::brood(&mut self.clouds, &sims, &self.bake.hosts, &self.bake.points);
            }
        }
        wake
    }

    fn run(&mut self, seconds: f64) {
        (0..(seconds * 30.0).round() as usize).for_each(|_| {
            self.frame(1.0 / 30.0);
        });
    }

    fn alive(&self, part: usize) -> Vec<Particle> {
        self.clouds[part].particles.iter().filter(|p| p.age < p.life).copied().collect()
    }
}

fn near(a: [f32; 2], b: [f32; 2], d: f32) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) <= d
}
