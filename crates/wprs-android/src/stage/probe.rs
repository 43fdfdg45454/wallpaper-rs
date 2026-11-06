use super::Stage;
use std::path::PathBuf;
use wprs_scene::{Course, ScriptOutputs};

impl Stage {
    pub(crate) fn shown(&mut self, shown: Vec<(String, PathBuf)>, frames: Vec<ScriptOutputs>) -> Vec<Course> {
        self.acting.keeping.shown = shown;
        self.gather();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !self.extras.lengths.is_empty() && std::time::Instant::now() < deadline {
            self.extras.timed(&mut self.runtime);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for outputs in frames {
            self.runtime.set_script(outputs);
            self.runtime.advance(0.1);
        }
        let mut courses = Vec::new();
        self.runtime.fill_videos(&mut courses);
        courses
    }
}
