use super::super::scripts::Scripts;
use super::SceneState;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use wprs_render::log_allowed;
use wprs_scene::{MAX_FIRED, ScriptOutputs};
use wprs_script::{Error, Frame, Mode, Setup};

impl SceneState {
    pub(in super::super) fn link(&mut self, scripts: &mut Scripts, output: &str) {
        if self.still || !self.runtime.uses().scripts {
            return;
        }
        let old = self.host.take();
        let stores = self.spec.pack.scene.scripts.iter().any(|s| s.wants(wprs_scene::ScriptApi::Storage));
        if stores && self.keeper.as_ref().is_none_or(|k| k.output() != output) {
            let (id, writes) = (&self.spec.pack.props.id, !crate::drafts::virtual_output(output));
            let dir = wprs_core::stash::dir();
            self.keeper = dir.and_then(|d| wprs_core::stash::Keeper::open(&d, id, output, writes));
        }
        let pack = &self.spec.pack;
        let values = serde_json::to_string(&self.spec.values).unwrap_or_default();
        let key = (Arc::as_ptr(pack) as usize, if stores { format!("{values} {output}") } else { values });
        let named = (pack.locale.as_str(), &pack.labels);
        let (keeper, folders) = (&self.keeper, self.folders());
        let setup = || {
            let mut setup = Setup::new(&pack.scene, &pack.scripts, self.runtime.seed, named);
            setup.storage = keeper.as_ref().map_or(serde_json::Value::Null, wprs_core::stash::Keeper::scopes);
            setup.folders = folders;
            setup
        };
        let (link, kept) = match old {
            Some(link) => link.rekey(scripts, key, setup),
            None => (super::super::scripts::link(scripts, key, setup), false),
        };
        if !kept {
            (self.answered, self.awaiting, self.timer) = (false, 0, f64::INFINITY);
            self.keeper.iter_mut().for_each(wprs_core::stash::Keeper::resend);
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
        let now = Instant::now();
        let listed = self.listing(now);
        self.keeper.iter_mut().for_each(|k| k.refresh(now));
        let storage = self.keeper.as_ref().map_or((0, ""), wprs_core::stash::Keeper::frame);
        let rt = &self.runtime;
        let minute = rt.wall().div_euclid(60.0) as i64;
        if self.zone.0 != minute {
            self.zone = (minute, crate::zone::posix());
        }
        let actions = std::mem::take(&mut self.pressed);
        let touches = std::mem::take(&mut self.touches);
        if touches.iter().any(|t| t.kind == wprs_scene::TouchKind::Click) {
            self.clicked[0] = Some(Instant::now());
        }
        let (view, screen, fps) = (self.sent.1, self.sent.2, self.sent.3);
        let eventful = listed
            || !self.fired.is_empty()
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
            folders: &self.listed.text,
            storage,
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
                self.open(output, std::mem::take(&mut outputs.open));
                self.show(&outputs);
                if let (Some(keeper), Some(stored)) = (self.keeper.as_mut(), outputs.storage.take()) {
                    keeper.take(&stored, now);
                }
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
}
