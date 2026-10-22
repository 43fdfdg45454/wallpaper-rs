use crate::frame::write_into;
use crate::shared::{Shared, Slot};
use crate::{memfd, params};
use pipewire as pw;
use pw::spa;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;

pub fn listen(
    stream: &pw::stream::StreamRc,
    shared: Arc<Shared>,
    slot: Arc<Slot>,
    output: &str,
    width: u32,
    height: u32,
) -> Result<pw::stream::StreamListener<()>, pw::Error> {
    let (output, stride, frames) = (output.to_owned(), width * 4, slot.clone());
    stream
        .add_local_listener::<()>()
        .state_changed(move |stream, _, old, new| {
            tracing::debug!(output = %output, "stream state {old:?} -> {new:?}");
            let streaming = matches!(new, pw::stream::StreamState::Streaming);
            if slot.streaming.swap(streaming, SeqCst) != streaming {
                shared.changed.store(true, SeqCst);
            }
            if streaming {
                let _ = stream.trigger_process();
            }
        })
        .param_changed(move |stream, _, id, param| {
            if id != spa::param::ParamType::Format.as_raw() || param.is_none() {
                return;
            }
            let bytes = params::buffers(width, height);
            if let Some(pod) = spa::pod::Pod::from_bytes(&bytes)
                && let Err(e) = stream.update_params(&mut [pod])
            {
                tracing::warn!("cannot set buffer parameters: {e}");
            }
        })
        .add_buffer(move |_, _, buffer| {
            if let Err(e) = unsafe { memfd::add(buffer, stride * height) } {
                tracing::warn!("cannot allocate a stream buffer: {e}");
            }
        })
        .remove_buffer(|_, _, buffer| unsafe { memfd::remove(buffer) })
        .process(move |stream, _| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                frames.dirty.store(false, SeqCst);
                write_into(buffer.datas_mut(), frames.frame().as_deref(), width, height);
            }
        })
        .register()
}
