use super::SceneView;
use super::clock::{Clock, Timing, charge};
use crate::readback::GPU_TIMEOUT;
use crate::{RenderError, Renderer};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::time::Instant;
use wprs_scene::Frame;

type Refused = BTreeMap<String, String>;

pub(super) fn refuse(refused: &mut Refused, frame: &Frame<'_>, charged: Vec<(u64, String)>) {
    for (key, why) in charged {
        charge(key, &why);
        let mut ids: Vec<&str> = frame
            .effects
            .iter()
            .filter(|e| e.descriptor.key == key)
            .map(|e| e.descriptor.id.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            if let Entry::Vacant(left) = refused.entry(id.to_owned()) {
                tracing::warn!("the effect {id} was left out: {why}");
                left.insert(why.clone());
            }
        }
    }
}

pub(super) fn stuck(refused: &mut Refused, frame: &Frame<'_>, keys: Vec<u64>) -> RenderError {
    let seconds = GPU_TIMEOUT.as_secs();
    let why = format!("it kept the graphics card busy for more than {seconds} s");
    if let Err(e) = wprs_scene::strike(&keys) {
        tracing::warn!("cannot remember the effects that kept the GPU busy: {e}");
    }
    refuse(refused, frame, keys.into_iter().map(|k| (k, why.clone())).collect());
    RenderError::DeviceLost(format!("a frame kept the GPU busy for more than {seconds} s"))
}

impl Renderer {
    pub(super) fn judge(&self, view: &mut SceneView, frame: &Frame<'_>) {
        if super::flight::flying(view.id) > 0 {
            drop(self.poll(wgpu::PollType::Poll));
        }
        let charged = view.clock.collect();
        refuse(&mut view.refused, frame, charged);
    }

    pub(super) fn quiet(&self, timing: Timing) -> bool {
        let wait = wgpu::PollType::Wait { submission_index: None, timeout: Some(GPU_TIMEOUT) };
        timing == Timing::Watch && matches!(self.poll(wait), crate::lost::Polled::Done)
    }

    pub(super) fn watch(
        &self,
        (clock, refused): (&mut Clock, &mut Refused),
        frame: &Frame<'_>,
        keys: &[u64],
        (index, at): (wgpu::SubmissionIndex, Instant),
    ) {
        let wait = wgpu::PollType::Wait { submission_index: Some(index), timeout: Some(GPU_TIMEOUT) };
        if matches!(self.poll(wait), crate::lost::Polled::Done) {
            let charged = clock.watched(keys, at.elapsed().as_secs_f64() * 1000.0);
            refuse(refused, frame, charged);
        }
    }
}
