use crate::dmabuf::{self, Allocate, Dmabuf};
use crate::frame::write_into;
use crate::memfd;
use crate::params::{self, Modifier};
use crate::shared::{Shared, Slot, lock};
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
    let (fixed, spares) = (Rc::new(Cell::new(None)), Rc::new(RefCell::new(Vec::new())));
    let (on_param, on_add, on_remove, frames) = (slot.clone(), slot.clone(), slot.clone(), slot.clone());
    let (chosen, kept, allocator, changed) = (fixed.clone(), spares.clone(), gpu.clone(), shared.clone());
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
                (_, _, Some(_)) => vec![params::buffers(width, height, fixed.get().is_some())],
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
            match made.is_some_and(|d| unsafe { dmabuf::add(buffer, d, height, &mut pool) }) {
                true => drop(on_add.frame().take()),
                false => {
                    tracing::warn!(
                        "cannot allocate a GPU stream buffer, recreating the stream in shared memory"
                    );
                    (pool.failed, pool.broken) = (Some(Instant::now()), true);
                }
            }
        })
        .remove_buffer(move |_, _, buffer| unsafe {
            dmabuf::remove(buffer, &mut lock(&on_remove.pool));
            memfd::remove(buffer);
        })
        .process(move |stream, _| {
            let starved = dmabuf::process(stream, &frames).unwrap_or_else(|| {
                let dirty = frames.dirty.load(SeqCst);
                let Some(mut buffer) = dirty.then(|| stream.dequeue_buffer()).flatten() else { return false };
                frames.dirty.store(false, SeqCst);
                let frame = frames.frame();
                write_into(buffer.datas_mut(), frame.as_deref(), width, height);
                frame.is_none()
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
