use crate::validate::{Check, Issue};

mod assets;
mod timing;

pub(crate) const IMAGES: [&str; 8] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "avif", "ktx2"];
pub(crate) const VIDEOS: [&str; 3] = ["webm", "mp4", "mkv"];

impl Check<'_> {
    pub(crate) fn canvas(&mut self) {
        let c = &self.scene.canvas;
        if let Some(f) = &c.parallax {
            self.uses(true, "parallax2");
            self.range("/canvas/parallax/amount", f.amount, 0.0, 4.0);
            self.range("/canvas/parallax/delay", f.delay, 0.0, 2.0);
            self.range("/canvas/parallax/pointer", f.pointer, 0.0, 1.0);
        }
        if let Some(cam) = &c.camera {
            self.uses(true, "camera");
            cam.center.iter().for_each(|v| self.value("/canvas/camera/center", v));
            self.number("/canvas/camera/zoom", &cam.zoom, 0.1, 10.0);
            self.number("/canvas/camera/rotation", &cam.rotation, -360.0, 360.0);
            if let Some(s) = &cam.shake {
                self.value("/canvas/camera/shake/enabled", &s.enabled);
                self.number("/canvas/camera/shake/amplitude", &s.amplitude, 0.0, 1000.0);
                self.number("/canvas/camera/shake/roughness", &s.roughness, 0.0, 1.0);
                self.number("/canvas/camera/shake/speed", &s.speed, 0.0, 60.0);
            }
        }
        if let Some(b) = &c.bloom {
            self.uses(true, "bloom");
            self.number("/canvas/bloom/threshold", &b.threshold, 0.0, 4.0);
            self.number("/canvas/bloom/strength", &b.strength, 0.0, 8.0);
            self.number("/canvas/bloom/radius", &b.radius, 0.0, 128.0);
            self.value("/canvas/bloom/tint", &b.tint);
            if let Some(h) = b.hdr {
                self.range("/canvas/bloom/hdr/threshold", h.threshold, 0.0, 16.0);
                self.range("/canvas/bloom/hdr/strength", h.strength, 0.0, 8.0);
            }
        }
        self.uses(c.hdr || c.tonemap.is_some(), "hdr");
        if c.tonemap.is_some() && !c.hdr {
            self.issues.push(Issue::Type("/canvas/tonemap".into()));
        }
        self.uses(!c.clear, "keep_frame");
        if let Some(d) = c.perspective {
            self.uses(true, "tilt");
            self.range("/canvas/perspective", d, 100.0, 1e5);
        }
        for (name, color) in [("ambient", c.ambient), ("sky", c.sky)] {
            let Some(color) = color else { continue };
            self.uses(true, "ambient_light");
            self.color(&format!("/canvas/{name}"), color);
        }
        if let Some(step) = self.scene.clock.step {
            self.uses(true, "clock_values");
            self.range("/clock/step", step, 1.0, 3600.0);
        }
        if let Some(a) = &self.scene.audio {
            self.uses(true, "audio_response");
            self.range("/audio/attack", a.attack, 0.001, 2.0);
            self.range("/audio/release", a.release, 0.01, 10.0);
        }
    }
}
