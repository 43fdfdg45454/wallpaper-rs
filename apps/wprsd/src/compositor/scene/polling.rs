use super::super::scripts::Scripts;
use super::SceneState;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use wprs_render::log_allowed;
use wprs_scene::{MAX_FIRED, ScriptOutputs};
use wprs_script::{Error, Frame, Mode, Setup};

const KEPT_LINES: usize = 50;

impl SceneState {
    pub(in super::super) fn link(&mut self, scripts: &mut Scripts) {
        if self.still || !self.runtime.uses().scripts {
            return;
        }
        let old = self.host.take();
        let pack = &self.spec.pack;
        let key = (Arc::as_ptr(pack) as usize, serde_json::to_string(&self.spec.values).unwrap_or_default());
        let named = (pack.locale.as_str(), &pack.labels);
        let setup = || Setup::new(&pack.scene, &pack.scripts, self.runtime.seed, named);
        let (link, kept) = match old {
            Some(link) => link.rekey(scripts, key, setup),
            None => (super::super::scripts::link(scripts, key, setup), false),
        };
        if !kept {
            (self.answered, self.awaiting, self.timer) = (false, 0, f64::INFINITY);
        }
        self.host = Some(link);
    }

    pub(in super::super) fn script(&mut self, output: &str, mode: Mode) -> Option<ScriptOutputs> {
        let ended = self.runtime.take_ended();
        let room = MAX_FIRED.saturating_sub(self.ended.len());
        self.ended.extend(ended.into_iter().take(room));
        let fresh = self.runtime.take_fired();
        let room = MAX_FIRED.saturating_sub(self.fired.len());
        self.fired.extend(fresh.into_iter().take(room));
        let rt = &self.runtime;
        let inside = rt.pointer().inside;
        let Some(host) = self.host.as_ref() else {
            self.pressed.clear();
            self.touches.clear();
            return None;
        };
        if host.leads(output, inside) {
            rt.fill_playbacks(&mut self.courses);
            rt.fill_reads(&mut self.read_back);
            rt.fill_sounds(&mut self.sounding);
            rt.fill_videos(&mut self.filmed);
        }
        let minute = rt.wall().div_euclid(60.0) as i64;
        if self.zone.0 != minute {
            self.zone = (minute, crate::zone::posix());
        }
        let actions = std::mem::take(&mut self.pressed);
        let touches = std::mem::take(&mut self.touches);
        let (view, screen, fps) = (self.sent.1, self.sent.2, self.sent.3);
        let eventful = !self.fired.is_empty()
            || !actions.is_empty()
            || !touches.is_empty()
            || self.timer <= rt.time()
            || self.sent.0 != self.exposed;
        let audio = (self.reads.0 && self.uses.audio).then(|| rt.audio());
        let frame = || Frame {
            time: rt.time(),
            dt: rt.dt(),
            wall: rt.wall(),
            zone: self.zone.1.as_str(),
            props: &self.exposed,
            actions: &actions,
            audio,
            pointer: rt.pointer(),
            view,
            screen,
            mode,
            fps,
            quality: self.quality as u8,
            timelines: &self.courses,
            fired: &self.fired,
            reads: &self.read_back,
            touches: &touches,
            sounds: &self.sounding,
            videos: &self.filmed,
            ended: &self.ended,
        };
        let link = self.host.as_mut()?;
        let before = link.progress().0;
        let sent = link.poll(output, inside, frame);
        let frames = link.progress().0;
        match frames > before {
            true => {
                self.fired.clear();
                self.ended.clear();
                if eventful {
                    (self.calm, self.awaiting) = (0, frames);
                    self.sent.0.clone_from(&self.exposed);
                }
            }
            false => {
                let mut kept = actions;
                kept.append(&mut self.pressed);
                self.pressed = kept;
                let mut held = touches;
                held.append(&mut self.touches);
                held.truncate(wprs_scene::MAX_TOUCHES);
                self.touches = held;
            }
        }
        match sent {
            Ok(Some(mut outputs)) => {
                self.answered = true;
                self.heard_script(&mut outputs);
                Some(outputs)
            }
            Ok(None) => None,
            Err(Error::Disabled) => {
                tracing::warn!(
                    scene = self.spec.pack.props.id,
                    "the scene scripts failed too often and are off"
                );
                (self.host, self.error) = (None, Some(crate::words::scripts_off()));
                None
            }
            Err(e) => {
                static FAILURES: AtomicU64 = AtomicU64::new(0);
                if log_allowed(&FAILURES) {
                    tracing::warn!(scene = self.spec.pack.props.id, "scene script: {e}");
                }
                None
            }
        }
    }

    fn heard_script(&mut self, outputs: &mut ScriptOutputs) {
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
