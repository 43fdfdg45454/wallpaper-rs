use pipewire as pw;
use pw::node::{Node, NodeListener, NodeState};
use pw::types::ObjectType;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::thread::JoinHandle;

const STREAM: &str = "Stream/Output/Audio";

pub struct Others {
    quit: pw::channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

struct Seen {
    nodes: HashMap<u32, (Node, NodeListener)>,
    playing: BTreeSet<u32>,
    told: Option<bool>,
    tell: Box<dyn Fn(bool)>,
}

impl Seen {
    fn set(&mut self, id: u32, playing: bool) {
        match playing {
            true => self.playing.insert(id),
            false => self.playing.remove(&id),
        };
        let now = !self.playing.is_empty();
        if self.told.replace(now) != Some(now) {
            (self.tell)(now);
        }
    }
}

impl Others {
    pub fn watch(
        ours: impl Fn(u32) -> bool + Send + 'static,
        tell: impl Fn(bool) + Send + 'static,
    ) -> Result<Others, String> {
        let ((quit, rx), (ready_tx, ready)) = (pw::channel::channel(), mpsc::channel());
        let thread = std::thread::Builder::new()
            .name("wprs-others".into())
            .spawn(move || {
                if let Err(e) = run(rx, Box::new(ours), Box::new(tell), &ready_tx) {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| e.to_string())?;
        match ready.recv() {
            Ok(Ok(())) => Ok(Others { quit, thread: Some(thread) }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("the playback watcher stopped".into()),
        }
    }
}

impl Drop for Others {
    fn drop(&mut self) {
        let _ = self.quit.send(());
        self.thread.take().map(JoinHandle::join);
    }
}

fn run(
    rx: pw::channel::Receiver<()>,
    ours: Box<dyn Fn(u32) -> bool>,
    tell: Box<dyn Fn(bool)>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let registry = core.get_registry_rc()?;
    let seen =
        Rc::new(RefCell::new(Seen { nodes: HashMap::new(), playing: BTreeSet::new(), told: None, tell }));
    let running = Rc::new(Cell::new(true));
    let (added, removed, bound, lost) =
        (Rc::downgrade(&seen), seen.clone(), registry.clone(), running.clone());
    let _core = core
        .add_listener_local()
        .error(move |id, _, _, _| lost.set(lost.get() && id != pw::core::PW_ID_CORE))
        .register();
    let _listener = registry
        .add_listener_local()
        .global(move |global| {
            let value = |key| global.props.and_then(|p| p.get(key));
            let pid = value("application.process.id").and_then(|p| p.parse().ok());
            if global.type_ != ObjectType::Node
                || value("media.class") != Some(STREAM)
                || pid.is_some_and(&ours)
            {
                return;
            }
            let Ok(node) = bound.bind::<Node, _>(global) else { return };
            let (id, state): (u32, Weak<RefCell<Seen>>) = (global.id, added.clone());
            let listener = node
                .add_listener_local()
                .info(move |info| {
                    let running = matches!(info.state(), NodeState::Running);
                    let seen = state.upgrade();
                    seen.iter().filter_map(|s| s.try_borrow_mut().ok()).for_each(|mut s| s.set(id, running));
                })
                .register();
            if let Some(mut seen) = added.upgrade().as_ref().and_then(|s| s.try_borrow_mut().ok()) {
                seen.nodes.insert(id, (node, listener));
            }
        })
        .global_remove(move |id| {
            if let Ok(mut seen) = removed.try_borrow_mut() {
                seen.nodes.remove(&id);
                seen.set(id, false);
            }
        })
        .register();
    let keep = running.clone();
    let _quit = rx.attach(mainloop.loop_(), move |()| keep.set(false));
    let _ = ready.send(Ok(()));
    while running.get() {
        mainloop.loop_().iterate(pw::loop_::Timeout::Infinite);
    }
    if let Ok(mut seen) = seen.try_borrow_mut() {
        seen.playing.clear();
        seen.set(0, false);
    }
    Ok(())
}
