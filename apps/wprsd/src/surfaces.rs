use crate::daemon::Daemon;
use crate::streams::SURFACE;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};
use wprs_wayland::{Event, Session};

const SETTLE: Duration = Duration::from_millis(500);

struct Settle<T>(HashMap<String, (T, Instant)>);

impl<T> Settle<T> {
    fn push(&mut self, name: String, event: T, now: Instant) {
        self.0.insert(name, (event, now + SETTLE));
    }

    fn deadline(&self) -> Option<Instant> {
        self.0.values().map(|(_, at)| *at).min()
    }

    fn due(&mut self, now: Instant) -> Vec<T> {
        self.0.extract_if(|_, (_, at)| *at <= now).map(|(_, (event, _))| event).collect()
    }
}

fn placed(event: &Event) -> Option<&str> {
    match event {
        Event::Output(name, _) | Event::Gone(name) => Some(name),
        _ => None,
    }
}

pub fn start(session: Session, daemon: Arc<Daemon>) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();
    let (frame, repaint) = (daemon.compositor.framer(), daemon.compositor.repainter());
    let backdrop = daemon.compositor.backdropper();
    session.start(move |event| match event {
        Event::Frame(output) => frame(&output),
        Event::Exposed(output) => repaint(&output),
        Event::Backdrop(output) => backdrop(&output),
        Event::Pointer(output, pos, buttons) => crate::inputs::surface_pointer(&output, pos, buttons),
        event => drop(tx.send(event)),
    })?;
    let follow = move || {
        let (mut covers, mut settle) = (HashMap::new(), Settle(HashMap::new()));
        loop {
            let received = match settle.deadline() {
                Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match received {
                Ok(event) => match placed(&event).map(str::to_owned) {
                    Some(name) => settle.push(name, event, Instant::now()),
                    None => daemon.surface_event(&mut covers, event),
                },
                Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }
            settle.due(Instant::now()).into_iter().for_each(|event| daemon.surface_event(&mut covers, event));
        }
    };
    std::thread::Builder::new().name("wprs-present".into()).spawn(follow)?;
    Ok(())
}

impl Daemon {
    fn surface_event(&self, covers: &mut HashMap<String, u8>, event: Event) {
        self.follow_surface(covers, event)
            .unwrap_or_else(|e| tracing::warn!("cannot follow an output: {e:#}"))
    }

    fn follow_surface(&self, covers: &mut HashMap<String, u8>, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Output(output, window) => {
                let (size, screen, refresh) = (window.size(), crate::edits::screen_of(&window), window.hz());
                crate::inputs::window(&output, Some(window.clone()));
                self.compositor.attach(&output, window);
                let streamed = {
                    let mut st = self.state();
                    let info = st.outputs.entry(output.clone()).or_default();
                    (info.size, info.screen, info.refresh) = (size, Some(screen), refresh);
                    let streamed = info.claims.keys().any(|claim| claim != SURFACE);
                    info.claims.retain(|claim, _| claim == SURFACE);
                    info.claims.entry(SURFACE.into()).or_default();
                    streamed
                };
                if streamed {
                    tracing::warn!(output, "closing a stream named after a screen that wprsd shows itself");
                    self.exporter.remove_stream(&output)?;
                }
                self.configure_all();
                self.set_covered(&output, SURFACE, covers.get(&output).is_some_and(|c| *c != 0))
            }
            Event::Covered(output, cover, covered) => {
                let bits = covers.entry(output.clone()).or_default();
                *bits = (*bits & !(1 << cover as u8)) | (u8::from(covered) << cover as u8);
                self.set_covered(&output, SURFACE, *bits != 0)
            }
            Event::Gone(output) => {
                covers.remove(&output);
                crate::inputs::window(&output, None);
                self.unclaim(&mut self.state(), &output, SURFACE, u32::MAX)?;
                tracing::info!(output, "output removed");
                self.configure_all();
                Ok(())
            }
            Event::Lost(reason) => {
                tracing::info!("the display server closed the connection ({reason}); exiting");
                crate::quit(0)
            }
            Event::Failed(reason) => {
                crate::fatal(&format!("the display server reported an error ({reason})"))
            }
            Event::Frame(_) | Event::Exposed(_) | Event::Backdrop(_) | Event::Pointer(..) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_changes_apply_once_they_settle() {
        let (mut settle, now) = (Settle(HashMap::new()), Instant::now());
        settle.push("DP-1".into(), "gone", now);
        settle.push("DP-2".into(), "added", now);
        settle.push("DP-1".into(), "back", now + SETTLE / 2);
        assert!(settle.due(now + SETTLE / 4).is_empty());
        assert_eq!(settle.due(now + SETTLE), ["added"]);
        assert_eq!(settle.deadline(), Some(now + SETTLE * 3 / 2));
        assert_eq!(settle.due(now + SETTLE * 2), ["back"]);
        assert_eq!(settle.deadline(), None);
    }
}
