use std::time::Instant;
use wprs_render::{GpuImage, Meter, RenderError, Renderer};
use wprs_scene::{Calming, FlashGuard};

pub(super) struct Guard {
    guard: FlashGuard,
    luma: Vec<f32>,
    meter: Option<Meter>,
    since: Instant,
}

impl Guard {
    pub(super) fn new(now: Instant) -> Box<Guard> {
        Box::new(Guard { guard: FlashGuard::default(), luma: Vec::new(), meter: None, since: now })
    }

    pub(super) fn watch(
        &mut self,
        r: &Renderer,
        image: Option<&GpuImage>,
        now: Instant,
        scene: &str,
    ) -> Result<(), RenderError> {
        let meter = self.meter.get_or_insert_with(|| r.meter());
        if !r.luma(meter, image, &mut self.luma)? {
            return Ok(());
        }
        let before = self.guard.calming();
        let after = self.guard.watch(now.duration_since(self.since).as_secs_f64(), &self.luma);
        match after {
            _ if after == before => {}
            Calming::Slowed(speed) => {
                tracing::warn!(
                    scene,
                    speed,
                    "the scene flashed more than three times a second and plays slower"
                )
            }
            Calming::Paused => {
                tracing::warn!(scene, "the scene kept flashing more than three times a second and stopped")
            }
            Calming::Steady => {}
        }
        Ok(())
    }

    pub(super) fn speed(&self) -> f64 {
        self.guard.speed()
    }

    pub(super) fn label(&self) -> &'static str {
        match self.guard.calming() {
            Calming::Steady => "",
            Calming::Slowed(_) => "slowed",
            Calming::Paused => "paused",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::output::{Output, Shown, Sink};
    use super::super::power::Power;
    use super::super::scene::SceneState;
    use super::super::{Content, SceneSpec};
    use crate::scenes::ScenePack;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use wprs_render::Renderer;
    use wprs_scene::{SCHEMA, Scene, Tier};

    fn state(layers: serde_json::Value, poster: f32) -> SceneState {
        staged(layers, json!({}), poster)
    }

    fn staged(layers: serde_json::Value, particles: serde_json::Value, poster: f32) -> SceneState {
        let timelines = [json!({ "id": "blink", "duration": 0.1, "loop": "repeat" })];
        let clock = json!({ "max_fps": 60, "poster_time": poster });
        let canvas = json!({ "size": [32, 18], "background": "#000000" });
        let text = json!({ "schema": SCHEMA, "canvas": canvas, "clock": clock, "timelines": timelines, "layers": layers,
            "particles": particles });
        let pack = Arc::new(ScenePack::bare(
            Scene::from_json(&text.to_string()).unwrap(),
            Default::default(),
            Vec::new(),
        ));
        let spec = SceneSpec {
            values: Default::default(),
            pack,
            extra: Vec::new(),
            clips: Vec::new(),
            hold: None,
            moving: false,
        };
        SceneState::new(spec, Power::new(Tier::T3, false, 60))
    }

    fn blinking() -> serde_json::Value {
        let opacity = json!({ "timeline": "blink", "keys": [[0, 0], [0.05, 1], [0.1, 0]] });
        json!([{ "id": "flash", "type": "solid", "color": "#ffffff", "opacity": opacity }])
    }

    #[test]
    fn the_state_says_where_particles_run() {
        let Ok(renderer) = Renderer::shared() else {
            return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required");
        };
        let dust = json!({ "dust": { "capacity": 50, "emitters": [{ "shape": { "point": [16, 9] }, "rate": 30 }] } });
        let mut scene = staged(json!([{ "id": "dust", "type": "particles", "system": "dust" }]), dust, 0.0);
        let mut plain = state(blinking(), 0.0);
        assert_eq!((scene.particles(), plain.particles()), ("", ""));
        let (power, at) = (Power::new(Tier::T3, false, 60), Instant::now());
        scene.render(renderer, ((32, 18), 60), power, &HashMap::new(), true, at).unwrap();
        plain.render(renderer, ((32, 18), 60), power, &HashMap::new(), true, at).unwrap();
        let path = if renderer.compute() && !renderer.software() { "gpu" } else { "cpu" };
        assert_eq!((scene.particles(), plain.particles()), (path, ""));
    }

    #[test]
    fn the_flash_guard_slows_a_flashing_scene_only_when_asked() {
        let Ok(renderer) = Renderer::shared() else {
            return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required");
        };
        let power = Power::new(Tier::T3, false, 60);
        let played = |on: bool| {
            let (mut scene, start) = (state(blinking(), 0.0), Instant::now());
            scene.guard(on);
            for n in 0..240 {
                let at = start + Duration::from_millis(n * 1000 / 60);
                scene.render(renderer, ((32, 18), 60), power, &HashMap::new(), true, at).unwrap();
            }
            (scene.time(), scene.flashes())
        };
        let (free, label) = played(false);
        assert!(free > 3.9 && label.is_empty(), "{free} {label}");
        let (guarded, label) = played(true);
        assert!(guarded < 0.7 * free && !label.is_empty(), "{guarded} {label}");
    }

    #[test]
    fn a_package_allowed_to_move_ignores_reduced_motion_until_the_choice_is_withdrawn() {
        let mut scene = state(blinking(), 0.03);
        let moving = SceneSpec { moving: true, ..scene.spec.clone() };
        scene.update(moving.clone());
        scene.calm(true);
        assert!(!scene.hushed && scene.quiet);
        let still = SceneSpec { moving: false, ..moving.clone() };
        assert!(scene.update(still) && scene.hushed);
        assert!(scene.update(moving) && !scene.hushed && scene.quiet);
    }

    #[test]
    fn reduced_motion_holds_the_poster_frame_and_wakes_at_most_once_a_minute() {
        let Ok(renderer) = Renderer::shared() else {
            return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required");
        };
        let (mut scene, start) = (state(blinking(), 0.03), Instant::now());
        let power = Power::new(Tier::T3, false, 60);
        assert!(scene.calm(true) && !scene.calm(true));
        for n in 0..5 {
            let at = start + Duration::from_millis(n * 100);
            scene.render(renderer, ((32, 18), 60), power, &HashMap::new(), true, at).unwrap();
            assert!((scene.time() - 0.03).abs() < 1e-6, "the scene moved to {}", scene.time());
            assert!(!scene.moving && scene.due.is_some_and(|due| due <= at + Duration::from_secs(60)));
        }
        assert!(scene.calm(false));
        scene
            .render(renderer, ((32, 18), 60), power, &HashMap::new(), true, start + Duration::from_secs(1))
            .unwrap();
        scene
            .render(renderer, ((32, 18), 60), power, &HashMap::new(), true, start + Duration::from_secs(2))
            .unwrap();
        assert!(scene.time() > 0.9 && scene.moving);
    }

    #[test]
    fn turning_the_guard_off_wakes_a_scene_it_paused() {
        let Ok(renderer) = Renderer::shared() else {
            return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required");
        };
        let (power, start, size) = (Power::new(Tier::T3, false, 60), Instant::now(), ((32, 18), 60));
        let keys = json!([[0, 0], [0.025, 1], [0.05, 0], [0.075, 1], [0.1, 0]]);
        let strobe = json!([{ "id": "flash", "type": "solid", "color": "#ffffff", "opacity": { "timeline": "blink", "keys": keys } }]);
        let mut scene = state(strobe, 0.0);
        scene.guard(true);
        let mut n = 0;
        while scene.flashes() != "paused" && n < 1200 {
            n += 1;
            let at = start + Duration::from_millis(n * 1000 / 60);
            scene.render(renderer, size, power, &HashMap::new(), true, at).unwrap();
        }
        assert!(scene.flashes() == "paused" && scene.due.is_none(), "{} after {n} frames", scene.flashes());
        let (fit, background) = (wprs_core::Fit::Stretch, [0; 3]);
        let (image, transition, uv, relief, scene) = (None, None, None, None, Some(scene));
        let shown = Shown { content: Content::Color, fit, background, image, transition, uv, relief, scene };
        let mut output = Output::new((32, 18), shown, Sink::Stream(None), 60);
        output.dirty = false;
        output.guard(true);
        assert!(!output.dirty, "keeping the guard on woke the paused scene");
        output.guard(false);
        assert!(output.dirty, "turning the guard off left the paused scene asleep");
        let scene = output.shown.scene.as_mut().expect("a scene");
        let (paused, later) = (scene.time(), start + Duration::from_millis(n * 1000 / 60));
        for k in 1..=30 {
            scene
                .render(renderer, size, power, &HashMap::new(), true, later + Duration::from_millis(k * 16))
                .unwrap();
        }
        assert!(scene.time() > paused + 0.3 && scene.due.is_some(), "{paused} {}", scene.time());
    }
}
