mod handlers;
mod hyprland;
mod pacer;
mod scale;
mod state;
mod toplevel;
mod window;

pub use pacer::Pacer;
pub use window::{Display, Window};

use smithay_client_toolkit::reexports::client::globals::{GlobalList, registry_queue_init};
use smithay_client_toolkit::reexports::client::{Connection, EventQueue};
use state::State;
use std::sync::Arc;

pub enum Event {
    Output(String, Window),
    Gone(String),
    Frame(String),
    Covered(String, Cover, bool),
    Lost(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cover {
    Starved,
    Toplevel,
    Hyprland,
}

pub(crate) type Sink = Arc<dyn Fn(Event) + Send + Sync>;

pub struct Session {
    conn: Connection,
    globals: GlobalList,
    queue: EventQueue<State>,
}

pub fn connect() -> anyhow::Result<Session> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<State>(&conn)?;
    let offered = |list: &[_]| list.iter().any(|g: &_| state::is_layer_shell(g));
    anyhow::ensure!(
        globals.contents().with_list(offered),
        "the compositor does not offer layer-shell surfaces"
    );
    Ok(Session { conn, globals, queue })
}

impl Session {
    pub fn display(&self) -> Display {
        Display(self.conn.clone())
    }

    pub fn start(self, sink: impl Fn(Event) + Send + Sync + 'static) -> anyhow::Result<()> {
        let Session { conn, globals, mut queue } = self;
        let sink: Sink = Arc::new(sink);
        let mut state = State::new(conn, &globals, &queue.handle(), sink.clone())?;
        hyprland::watch(sink);
        std::thread::Builder::new().name("wprs-wayland".into()).spawn(move || {
            let error = loop {
                if let Err(e) = queue.blocking_dispatch(&mut state) {
                    break e;
                }
            };
            (state.sink)(Event::Lost(error.to_string()))
        })?;
        Ok(())
    }
}
