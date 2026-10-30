use crate::listen::{Listen, Taps};
use crate::{Analyzer, AudioFrame};
use pipewire as pw;
use pw::loop_::Timeout;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const RING: usize = 16_384;
const RETRY_MIN: Duration = Duration::from_millis(500);
const RETRY_MAX: Duration = Duration::from_secs(30);

pub struct Capture {
    quit: pw::channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

pub struct Feed {
    samples: rtrb::Consumer<f32>,
    rate: Arc<AtomicU32>,
}

impl Feed {
    pub fn rate(&self) -> u32 {
        self.rate.load(Relaxed)
    }

    pub fn drain(&mut self, mut sink: impl FnMut(&[f32])) {
        let Ok(chunk) = self.samples.read_chunk(self.samples.slots() & !1) else { return };
        let (a, b) = chunk.as_slices();
        sink(a);
        sink(b);
        chunk.commit_all();
    }
}

impl Capture {
    pub fn start(wake: impl Fn() + Send + 'static) -> Result<(Capture, Feed), String> {
        let (producer, samples) = rtrb::RingBuffer::new(RING);
        let (rate, (quit, rx), (ready_tx, ready)) =
            (Arc::new(AtomicU32::new(48_000)), pw::channel::channel(), mpsc::channel());
        let shared = rate.clone();
        let thread = std::thread::Builder::new()
            .name("wprs-audio".into())
            .spawn(move || {
                let taps = Taps::new(producer, shared, Box::new(wake));
                if let Err(e) = run(rx, taps, &ready_tx) {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| e.to_string())?;
        let failure = match ready.recv() {
            Ok(Ok(())) => return Ok((Capture { quit, thread: Some(thread) }, Feed { samples, rate })),
            Ok(Err(e)) => e,
            Err(_) => "the audio thread stopped".into(),
        };
        let _ = thread.join();
        Err(failure)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.quit.send(());
        self.thread.take().map(JoinHandle::join);
    }
}

fn run(
    rx: pw::channel::Receiver<()>,
    taps: Taps,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let (running, taps) = (Rc::new(Cell::new(true)), Rc::new(RefCell::new(taps)));
    let keep = running.clone();
    let _quit = rx.attach(mainloop.loop_(), move |()| keep.set(false));
    let (mut failures, mut opened) = (0, Instant::now());
    let mut link =
        Listen::open(&context, &taps).inspect_err(|e| tracing::debug!("cannot capture audio yet: {e}")).ok();
    let mut retry = link.is_none().then(|| next(&mut failures));
    let _ = ready.send(Ok(()));
    while running.get() {
        let wait = retry
            .map(|at: Instant| at.saturating_duration_since(Instant::now()).max(Duration::from_millis(1)));
        mainloop.loop_().iterate(wait.map_or(Timeout::Infinite, Timeout::Finite));
        if link.as_ref().is_some_and(Listen::retarget) {
            tracing::debug!("an audio output appeared, capturing its sound");
            (link, retry) = (None, Some(Instant::now()));
        }
        if link.as_ref().is_some_and(Listen::broken) {
            failures = if opened.elapsed() > RETRY_MAX { 0 } else { failures };
            tracing::info!("the audio capture lost PipeWire, connecting again");
            (link, retry) = (None, Some(next(&mut failures)));
        }
        if link.is_none() && retry.is_some_and(|at| at <= Instant::now()) {
            match Listen::open(&context, &taps) {
                Ok(open) => {
                    tracing::info!("the audio capture is connected to PipeWire again");
                    (link, retry, opened) = (Some(open), None, Instant::now());
                }
                Err(e) => {
                    tracing::debug!(failures, "cannot capture audio yet: {e}");
                    retry = Some(next(&mut failures));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn backoff(failures: u32) -> Duration {
    RETRY_MAX.min(RETRY_MIN * 2u32.pow(failures.min(6)))
}

fn next(failures: &mut u32) -> Instant {
    *failures += 1;
    Instant::now() + backoff(*failures - 1)
}

pub struct Audio {
    analyzer: Analyzer,
    feed: Option<Feed>,
    last: Option<Instant>,
}

impl Default for Audio {
    fn default() -> Self {
        Audio { analyzer: Analyzer::new(48_000), feed: None, last: None }
    }
}

impl Audio {
    pub fn attach(&mut self, feed: Option<Feed>) {
        *self = Audio { feed, ..Audio::default() };
    }

    pub fn attached(&self) -> bool {
        self.feed.is_some()
    }

    pub fn frame(&mut self) -> AudioFrame {
        let Some(feed) = &mut self.feed else { return AudioFrame::default() };
        self.analyzer.set_rate(feed.rate());
        feed.drain(|samples| self.analyzer.push(samples));
        let now = Instant::now();
        let dt = self.last.replace(now).map_or(0.0, |last| (now - last).as_secs_f32());
        self.analyzer.update(dt).clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reconnecting_waits_longer_each_time_up_to_half_a_minute() {
        let delays: Vec<_> = (0..9).map(|n| super::backoff(n).as_millis()).collect();
        assert_eq!(delays, [500, 1000, 2000, 4000, 8000, 16_000, 30_000, 30_000, 30_000]);
    }
}
