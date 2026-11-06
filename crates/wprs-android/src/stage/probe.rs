use super::Stage;
use std::path::PathBuf;
use wprs_scene::{Course, ScriptOutputs};

impl Stage {
    pub(crate) fn shown(&mut self, shown: Vec<(String, PathBuf)>, frames: Vec<ScriptOutputs>) -> Vec<Course> {
        self.acting.keeping.shown = shown;
        self.gather();
        for outputs in frames {
            self.runtime.set_script(outputs);
            self.runtime.advance(0.1);
        }
        let mut courses = Vec::new();
        self.runtime.fill_videos(&mut courses);
        courses
    }
}
