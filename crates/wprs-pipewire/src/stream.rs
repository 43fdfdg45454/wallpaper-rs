use crate::frame::write_into;
use crate::{VideoFrame, memfd, params};
use pipewire as pw;
use pw::spa;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const KEEPALIVE: Duration = Duration::from_secs(10);

type SharedFrame = Rc<RefCell<Option<Arc<VideoFrame>>>>;

pub struct OutputStream {
    pub stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<()>,
    pub width: u32,
    pub height: u32,
    frame: SharedFrame,
    streaming: Rc<Cell<bool>>,
    last_trigger: Instant,
}

impl OutputStream {
    pub fn new(core: &pw::core::CoreRc, output: &str, width: u32, height: u32) -> Result<Self, pw::Error> {
        let name = format!("wallpaper-rs.{output}");
        let props = pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Source",
            *pw::keys::MEDIA_ROLE => "Screen",
            *pw::keys::NODE_NAME => name.as_str(),
            *pw::keys::NODE_DESCRIPTION => format!("wallpaper-rs ({output})"),
        };
        let stream = pw::stream::StreamRc::new(core.clone(), &name, props)?;
        let (frame, streaming) = (SharedFrame::default(), Rc::new(Cell::new(false)));
        let _listener = listen(&stream, output, width, height, frame.clone(), streaming.clone())?;
        let format = params::enum_format(width, height);
        let mut pods = [spa::pod::Pod::from_bytes(&format).expect("valid format pod")];
        let flags = pw::stream::StreamFlags::DRIVER | pw::stream::StreamFlags::ALLOC_BUFFERS;
        stream.connect(spa::utils::Direction::Output, None, flags, &mut pods)?;
        Ok(Self { stream, _listener, width, height, frame, streaming, last_trigger: Instant::now() })
    }

    pub fn show(&mut self, frame: Arc<VideoFrame>) {
        *self.frame.borrow_mut() = Some(frame);
        self.trigger();
    }

    pub fn keep_alive(&mut self) {
        if self.last_trigger.elapsed() >= KEEPALIVE {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        tracing::trace!(streaming = self.streaming.get(), "trigger");
        if self.streaming.get() {
            if let Err(e) = self.stream.trigger_process() {
                tracing::debug!("trigger_process failed: {e}");
            }
            self.last_trigger = Instant::now();
        }
    }
}

fn listen(
    stream: &pw::stream::StreamRc,
    output: &str,
    width: u32,
    height: u32,
    frame: SharedFrame,
    streaming: Rc<Cell<bool>>,
) -> Result<pw::stream::StreamListener<()>, pw::Error> {
    let (output, stride) = (output.to_owned(), width * 4);
    stream
        .add_local_listener::<()>()
        .state_changed(move |stream, _, old, new| {
            tracing::debug!(output = %output, "stream state {old:?} -> {new:?}");
            streaming.set(matches!(new, pw::stream::StreamState::Streaming));
            if streaming.get() {
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
                write_into(buffer.datas_mut(), frame.borrow().as_deref(), width, height);
            }
        })
        .register()
}
