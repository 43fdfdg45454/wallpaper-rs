use super::SceneState;
use std::time::{Duration, Instant};
use wprs_scene::ScriptOutputs;

const KEPT_LINES: usize = 50;
const CLICK: Duration = Duration::from_secs(2);
const APART: Duration = Duration::from_secs(1);

impl SceneState {
    pub(super) fn open(&mut self, output: &str, asked: Vec<String>) {
        let now = Instant::now();
        let recent = self.clicked[0].is_some_and(|at| now.duration_since(at) < CLICK);
        let rested = self.clicked[1].is_none_or(|at| now.duration_since(at) >= APART);
        if let Some(prop) = asked.first().filter(|_| recent && rested && self.allowed[1]) {
            self.clicked = [None, Some(now)];
            crate::shortcuts::ask(output, &self.spec.pack.props.id, prop);
        }
    }

    pub(super) fn heard_script(&mut self, outputs: &mut ScriptOutputs) {
        for line in outputs.console.drain(..) {
            let line = wprs_scene::plain_line(&line);
            tracing::debug!(scene = self.spec.pack.props.id, "script {line}");
            self.console.push_back(line);
        }
        while self.console.len() > KEPT_LINES {
            self.console.pop_front();
        }
        let stopped =
            outputs.stopped.last().map(|why| crate::words::script_stopped(&wprs_scene::plain_line(why)));
        if stopped.is_some() && stopped != self.error {
            tracing::warn!(
                scene = self.spec.pack.props.id,
                "a scene script stopped: {}",
                outputs.stopped.iter().map(|s| wprs_scene::plain_line(s)).collect::<Vec<_>>().join("; ")
            );
            self.error = stopped;
        }
        self.timer = outputs.wake.map_or(f64::INFINITY, |w| self.runtime.time() + w);
    }
}
