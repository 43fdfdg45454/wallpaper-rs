use crate::Command;
use crate::link::Link;
use crate::shared::Shared;
use crate::state::State;
use pipewire as pw;
use pw::loop_::Timeout;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

const RETRY_MIN: Duration = Duration::from_millis(500);
const RETRY_MAX: Duration = Duration::from_secs(5);

pub fn run(
    rx: pw::channel::Receiver<Command>,
    shared: Arc<Shared>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let state = Rc::new(RefCell::new(State::new(shared.clone())));
    let mut link = Some(Link::open(&context, &state)?);

    let (on_command, running) = (state.clone(), Rc::new(Cell::new(true)));
    let keep_running = running.clone();
    let _receiver = rx.attach(mainloop.loop_(), move |command| match command {
        Command::Quit => keep_running.set(false),
        Command::Ensure { slot, output, width, height, reply } => {
            on_command.borrow_mut().ensure(slot, output, width, height, reply)
        }
        Command::Remove { output } => on_command.borrow_mut().remove(&output),
        Command::Wake => on_command.borrow_mut().wake(),
    });

    let _ = ready.send(Ok(()));
    let (mut failures, mut retry_at, mut opened) = (0, None, Instant::now());
    while running.get() {
        let deadline = state.borrow().deadline().into_iter().chain(retry_at).min();
        mainloop.loop_().iterate(timeout(deadline));
        if link.as_ref().is_some_and(Link::broken) {
            state.borrow_mut().disconnect();
            if opened.elapsed() > RETRY_MAX {
                failures = 0;
            }
            (link, retry_at) = (None, Some(next_retry(&mut failures)));
        }
        if link.is_none() && retry_at.is_some_and(|at| at <= Instant::now()) {
            match Link::open(&context, &state) {
                Ok(open) => {
                    tracing::info!(generation = shared.generation.load(SeqCst), "reconnected to PipeWire");
                    (link, retry_at, opened) = (Some(open), None, Instant::now());
                }
                Err(e) => {
                    retry_at = Some(next_retry(&mut failures));
                    tracing::debug!(failures, "cannot reconnect to PipeWire: {e}");
                }
            }
        }
        state.borrow_mut().tick();
        shared.flush();
    }
    state.borrow_mut().close();
    Ok(())
}

fn backoff(failures: u32) -> Duration {
    RETRY_MAX.min(RETRY_MIN * 2u32.pow(failures.min(4)))
}

fn next_retry(failures: &mut u32) -> Instant {
    *failures += 1;
    Instant::now() + backoff(*failures - 1)
}

fn timeout(deadline: Option<Instant>) -> Timeout {
    let wait = |at: Instant| at.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
    deadline.map_or(Timeout::Infinite, |at| Timeout::Finite(wait(at)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_five_seconds() {
        let delays: Vec<_> = (0..7).map(|n| backoff(n).as_millis()).collect();
        assert_eq!(delays, [500, 1000, 2000, 4000, 5000, 5000, 5000]);
    }

    #[test]
    fn timeouts_never_spin() {
        assert!(matches!(timeout(None), Timeout::Infinite));
        let past = Instant::now() - Duration::from_secs(1);
        assert!(matches!(timeout(Some(past)), Timeout::Finite(d) if d == Duration::from_millis(1)));
    }
}
