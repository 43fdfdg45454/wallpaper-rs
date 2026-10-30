use super::{Sent, flight, settle};
use crate::{Renderer, lost::Polled, readback::GPU_TIMEOUT};
use std::time::Instant;

const CAP: usize = 2;

fn wait(r: &Renderer, index: wgpu::SubmissionIndex) -> bool {
    let wait = wgpu::PollType::Wait { submission_index: Some(index), timeout: Some(GPU_TIMEOUT) };
    !matches!(r.poll(wait), Polled::Timeout)
}

fn stuck(view: usize) -> Vec<u64> {
    let mut f = flight();
    let v = f.view(view);
    let mut keys: Vec<u64> = v.sent.iter().flat_map(|s| &s.keys).chain(&v.pending).copied().collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

pub(crate) fn sending(device: &Renderer, view: usize, keys: &[u64]) -> Result<u64, Vec<u64>> {
    loop {
        let (serial, index) = {
            let mut f = flight();
            let v = f.view(view);
            if v.sent.len() < CAP {
                v.serial += 1;
                let mut held = v.spare.pop().unwrap_or_default();
                held.extend_from_slice(keys);
                let sent = Sent { serial: v.serial, index: None, keys: held, at: Instant::now() };
                v.sent.push_back(sent);
                return Ok(v.serial);
            }
            (v.sent[0].serial, v.sent[0].index.clone())
        };
        if let Some(index) = index
            && !wait(device, index)
        {
            return Err(stuck(view));
        }
        done(view, serial);
    }
}

pub(crate) fn sent(
    device: &Renderer,
    view: usize,
    serial: u64,
    index: Option<wgpu::SubmissionIndex>,
) -> Result<(), Vec<u64>> {
    let trial = {
        let mut f = flight();
        let v = f.view(view);
        v.idle = false;
        let Some(sent) = v.sent.iter_mut().find(|s| s.serial == serial) else { return Ok(()) };
        sent.index.clone_from(&index);
        !v.pending.is_empty()
    };
    if let Some(index) = index {
        if !trial {
            return Ok(());
        }
        if !wait(device, index) {
            return Err(stuck(view));
        }
    }
    done(view, serial);
    Ok(())
}

pub(crate) fn done(view: usize, serial: u64) {
    let mut f = flight();
    let Some(v) = f.views.iter_mut().find(|v| v.id == view) else { return };
    let Some(at) = v.sent.iter().position(|s| s.serial == serial) else { return };
    if let Some(mut gone) = v.sent.remove(at) {
        gone.keys.clear();
        v.spare.push(gone.keys);
    }
    v.completed += 1;
    let first = v.first.is_none();
    v.first.get_or_insert_with(Instant::now);
    let settled = settle::settled(v);
    let later = first && !v.pending.is_empty();
    drop(f);
    if later {
        settle::later(view);
    }
    settled.into_iter().for_each(wprs_scene::tried);
}

pub(crate) fn idle(view: usize) {
    let mut f = flight();
    let v = f.view(view);
    v.idle = true;
    let quiet = v.completed > 0 && v.sent.is_empty();
    let settled = if quiet { std::mem::take(&mut v.pending) } else { Vec::new() };
    drop(f);
    settled.into_iter().for_each(wprs_scene::tried);
}
