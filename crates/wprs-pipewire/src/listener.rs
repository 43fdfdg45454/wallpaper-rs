use crate::dmabuf::{self, Allocate, Dmabuf};
use crate::params::{self, Modifier};
use crate::shared::{Due, Shared, Slot, lock};
use crate::{damage, memfd};
use pipewire as pw;
use pw::spa;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Instant;

const SPARES: usize = 4;

pub fn listen(
    stream: &pw::stream::StreamRc,
    shared: Arc<Shared>,
    slot: Arc<Slot>,
    output: &str,
    (width, height): (u32, u32),
) -> Result<pw::stream::StreamListener<()>, pw::Error> {
    let (output, stride, gpu) = (output.to_owned(), width * 4, shared.gpu.get().cloned());
    let timelines = shared.timelines.get().cloned();
    let synced = timelines.is_some();
    let (fixed, spares) = (Rc::new(Cell::new(None)), Rc::new(RefCell::new(Vec::new())));
    let (on_param, on_add, on_remove, frames) = (slot.clone(), slot.clone(), slot.clone(), slot.clone());
    let (chosen, kept, allocator, changed) = (fixed.clone(), spares.clone(), gpu.clone(), shared.clone());
    let seen = Rc::new(RefCell::new(damage::Seen::default()));
    let forgot = seen.clone();
    stream
        .add_local_listener::<()>()
        .state_changed(move |stream, _, old, new| {
            tracing::debug!(output = %output, "stream state {old:?} -> {new:?}");
            let streaming = matches!(new, pw::stream::StreamState::Streaming);
            if slot.streaming.swap(streaming, SeqCst) != streaming {
                shared.changed.store(true, SeqCst);
            }
            if streaming {
                *lock(&slot.started) = Some(Instant::now());
                slot.dirty.store(true, SeqCst);
                let _ = stream.trigger_process();
            }
        })
        .param_changed(move |stream, _, id, param| {
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let modifier = param.map_or(Modifier::None, params::modifier);
            fixed.set(match &modifier {
                Modifier::Fixed(m) => gpu.as_ref().map(|_| *m),
                _ => None,
            });
            if fixed.get().is_none() {
                spares.borrow_mut().clear();
            }
            let pods = match (modifier, gpu.as_deref(), param) {
                (_, _, None) => return,
                (Modifier::Choice(list), Some(gpu), _) => {
                    fixate(gpu, &on_param, (width, height), &list, &mut spares.borrow_mut())
                }
                (_, _, Some(_)) if synced && fixed.get().is_some() => {
                    params::synced(width, height).into_iter().chain([params::damage()]).collect()
                }
                (_, _, Some(_)) => {
                    vec![params::buffers(width, height, fixed.get().is_some()), params::damage()]
                }
            };
            let mut pods: Vec<&spa::pod::Pod> =
                pods.iter().filter_map(|b| spa::pod::Pod::from_bytes(b)).collect();
            if !pods.is_empty()
                && let Err(e) = stream.update_params(&mut pods)
            {
                tracing::warn!("cannot set stream parameters: {e}");
            }
        })
        .add_buffer(move |_, _, buffer| {
            let wanted =
                chosen.get().zip(allocator.as_deref()).filter(|_| unsafe { dmabuf::offered(buffer) });
            let Some((modifier, gpu)) = wanted else {
                lock(&on_add.pool).active = false;
                if let Err(e) = unsafe { memfd::add(buffer, stride * height) } {
                    tracing::warn!("cannot allocate a stream buffer: {e}");
                }
                return;
            };
            let spare = kept.borrow_mut().pop().filter(|d| d.modifier == modifier);
            let made = spare.or_else(|| gpu.allocate(width, height, &[modifier]));
            let mut pool = lock(&on_add.pool);
            match made.and_then(|d| unsafe { dmabuf::add(buffer, d, height, &mut pool) }) {
                Some(index) => {
                    drop(on_add.frame().take());
                    let fenced =
                        timelines.as_ref().map(|drm| unsafe { dmabuf::fence(buffer, drm, &mut pool, index) });
                    if let Some(Err(e)) = fenced {
                        tracing::warn!("cannot add explicit synchronization to a stream buffer: {e}");
                    }
                }
                None => {
                    tracing::warn!(
                        "cannot allocate a GPU stream buffer, recreating the stream in shared memory"
                    );
                    (pool.failed, pool.broken) = (Some(Instant::now()), true);
                }
            }
        })
        .remove_buffer(move |_, _, buffer| unsafe {
            forgot.borrow_mut().forget(buffer);
            dmabuf::remove(buffer, &mut lock(&on_remove.pool));
            memfd::remove(buffer);
        })
        .process(move |stream, _| {
            let size = (width, height);
            let starved = dmabuf::process(stream, &frames, size).unwrap_or_else(|| match frames.due() {
                Due::Missing => true,
                Due::Sent => false,
                Due::Ready(frame) => {
                    let buffer = unsafe { stream.dequeue_raw_buffer() };
                    if let (false, Some(frame)) = (buffer.is_null(), frame.as_deref()) {
                        frames.dirty.store(false, SeqCst);
                        let (ages, mut seen) = (lock(&frames.ages), seen.borrow_mut());
                        let held = seen.swap(buffer, ages.serial());
                        unsafe { damage::write(buffer, frame, size, ages.since(held)) };
                        unsafe { damage::mark(buffer, ages.since(seen.sent), size) };
                        seen.sent = ages.serial();
                        unsafe { stream.queue_raw_buffer(buffer) };
                    }
                    false
                }
            });
            changed.changed.fetch_or(starved, SeqCst);
        })
        .register()
}

fn fixate(
    gpu: &dyn Allocate,
    slot: &Slot,
    (width, height): (u32, u32),
    list: &[u64],
    spares: &mut Vec<Dmabuf>,
) -> Vec<Vec<u8>> {
    let first = gpu.allocate(width, height, list);
    let modifier = first.as_ref().map(|d| d.modifier);
    let rest =
        modifier.into_iter().flat_map(|m| (1..SPARES).map_while(move |_| gpu.allocate(width, height, &[m])));
    *spares = first.into_iter().chain(rest).collect();
    let mut pool = lock(&slot.pool);
    if spares.len() < SPARES {
        tracing::warn!("cannot allocate GPU stream buffers, falling back to shared memory");
        (*spares, pool.failed) = (Vec::new(), Some(Instant::now()));
        return params::formats(width, height, &[], None);
    }
    tracing::debug!(modifier = format!("{:#x}", spares[0].modifier), "stream buffers use GPU memory");
    pool.failed = None;
    params::formats(width, height, gpu.modifiers(), modifier)
}
