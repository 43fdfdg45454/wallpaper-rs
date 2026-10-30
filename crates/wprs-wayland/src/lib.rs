mod backdrop;
mod cosmic;
mod cover;
mod desk;
mod handlers;
mod hyprland;
mod modes;
mod monitors;
mod pacer;
mod pointer;
mod presented;
mod scale;
mod stacking;
mod state;
mod toplevel;
mod window;
mod x11;

pub use modes::watch_modes;
pub use pacer::Pacer;
pub use window::{Display, Window};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use smithay_client_toolkit::reexports::client::globals::{GlobalList, registry_queue_init};
use smithay_client_toolkit::reexports::client::{Connection, DispatchError, EventQueue};
use state::State;
use std::sync::Arc;
use std::time::Instant;
use wayland_backend::client::WaylandError;
use x11::X11;

pub enum Event {
    Output(String, Window),
    Gone(String),
    Frame(String),
    Covered(String, Cover, bool),
    Pointer(String, Option<[f32; 2]>, u8),
    Input(String),
    Exposed(String),
    Backdrop(String),
    Lost(String),
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cover {
    Starved,
    Toplevel,
    Hyprland,
    Overlap,
}

pub(crate) type Sink = Arc<dyn Fn(Event) + Send + Sync>;

pub struct Session(Kind);

enum Kind {
    Wayland(Connection, GlobalList, EventQueue<State>),
    X11(Arc<X11>),
}

pub fn connect() -> anyhow::Result<Session> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<State>(&conn)?;
    let offered = |list: &[_]| list.iter().any(|g: &_| state::is_layer_shell(g));
    anyhow::ensure!(
        globals.contents().with_list(offered),
        "the compositor does not offer layer-shell surfaces"
    );
    Ok(Session(Kind::Wayland(conn, globals, queue)))
}

pub fn connect_x11() -> anyhow::Result<Session> {
    Ok(Session(Kind::X11(X11::connect()?)))
}

impl Session {
    pub fn kind(&self) -> &'static str {
        match self.0 {
            Kind::Wayland(..) => "layer-shell",
            Kind::X11(_) => "x11",
        }
    }

    pub fn display(&self) -> Display {
        match &self.0 {
            Kind::Wayland(conn, ..) => Display::Wayland(conn.clone()),
            Kind::X11(x) => Display::X11(x.clone()),
        }
    }

    pub fn start(self, sink: impl Fn(Event) + Send + Sync + 'static) -> anyhow::Result<()> {
        let sink: Sink = Arc::new(sink);
        let (conn, globals, mut queue) = match self.0 {
            Kind::Wayland(conn, globals, queue) => (conn, globals, queue),
            Kind::X11(x) => return monitors::start(x, sink),
        };
        let mut state = State::new(conn, &globals, &queue.handle(), sink.clone())?;
        hyprland::watch(sink);
        std::thread::Builder::new().name("wprs-wayland".into()).spawn(move || {
            let error = loop {
                let retry = state.retry_at();
                if let Err(e) = dispatch(&mut queue, &mut state, retry) {
                    break e;
                }
                state.reopen();
            };
            (state.sink)(ended(error))
        })?;
        Ok(())
    }
}

fn dispatch<S>(
    queue: &mut EventQueue<S>,
    state: &mut S,
    until: Option<Instant>,
) -> Result<usize, DispatchError> {
    let Some(at) = until else { return queue.blocking_dispatch(state) };
    let dispatched = queue.dispatch_pending(state)?;
    if dispatched > 0 {
        return Ok(dispatched);
    }
    queue.flush()?;
    if let Some(guard) = queue.prepare_read() {
        let wait = Timespec::try_from(at.saturating_duration_since(Instant::now())).unwrap_or_default();
        let fd = guard.connection_fd();
        let ready = poll(&mut [PollFd::new(&fd, PollFlags::IN | PollFlags::ERR)], Some(&wait));
        if ready.is_ok_and(|n| n > 0) {
            match guard.read() {
                Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                read => drop(read?),
            }
        }
    }
    queue.dispatch_pending(state)
}

fn ended(error: DispatchError) -> Event {
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, NotConnected, UnexpectedEof};
    match error {
        DispatchError::Backend(WaylandError::Io(e))
            if matches!(
                e.kind(),
                BrokenPipe | ConnectionReset | ConnectionAborted | NotConnected | UnexpectedEof
            ) =>
        {
            Event::Lost(e.to_string())
        }
        error => Event::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayland_backend::protocol::ProtocolError;

    #[test]
    fn protocol_errors_fail_and_a_closed_connection_ends() {
        let (code, object_id, object_interface) = (2, 9, "zwlr_layer_surface_v1".into());
        let error = ProtocolError { code, object_id, object_interface, message: "invalid size".into() };
        assert!(matches!(ended(DispatchError::Backend(WaylandError::Protocol(error))), Event::Failed(_)));
        let io = |kind| DispatchError::Backend(WaylandError::Io(std::io::Error::from(kind)));
        assert!(matches!(ended(io(std::io::ErrorKind::BrokenPipe)), Event::Lost(_)));
        assert!(matches!(ended(io(std::io::ErrorKind::InvalidData)), Event::Failed(_)));
    }

    #[test]
    fn a_pending_retry_wakes_the_session_without_any_event() {
        let (ours, _compositor) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut queue = Connection::from_socket(ours).unwrap().new_event_queue::<()>();
        let at = Instant::now() + std::time::Duration::from_millis(50);
        assert_eq!(dispatch(&mut queue, &mut (), Some(at)).unwrap(), 0);
        assert!(Instant::now() >= at, "the session woke before the retry was due");
        assert_eq!(dispatch(&mut queue, &mut (), Some(at)).unwrap(), 0, "a due retry waited");
    }
}
