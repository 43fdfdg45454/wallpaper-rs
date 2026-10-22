use crate::state::State;
use pipewire as pw;
use rustix::io::Errno;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const FATAL: [Errno; 3] = [Errno::PIPE, Errno::CONNRESET, Errno::NOTCONN];

pub struct Link {
    _registry_listener: pw::registry::Listener,
    _core_listener: pw::core::Listener,
    _registry: pw::registry::RegistryRc,
    _core: pw::core::CoreRc,
    broken: Rc<Cell<bool>>,
}

impl Link {
    pub fn open(context: &pw::context::ContextRc, state: &Rc<RefCell<State>>) -> Result<Self, pw::Error> {
        let core = context.connect_rc(None)?;
        let broken = Rc::new(Cell::new(false));
        let on_error = broken.clone();
        let _core_listener = core
            .add_listener_local()
            .error(move |id, _, res, message| {
                if id == pw::core::PW_ID_CORE && FATAL.iter().any(|e| -e.raw_os_error() == res) {
                    tracing::warn!("PipeWire connection lost: {message}");
                    on_error.set(true);
                }
            })
            .register();
        let registry = core.get_registry_rc()?;
        let (on_global, on_global_remove) = (state.clone(), state.clone());
        let _registry_listener = registry
            .add_listener_local()
            .global(move |global| {
                if global.type_ == pw::types::ObjectType::Node
                    && let Some(serial) = global.props.and_then(|p| p.get("object.serial")?.parse().ok())
                    && let Ok(mut state) = on_global.try_borrow_mut()
                {
                    state.serials.insert(global.id, serial);
                    state.resolve();
                }
            })
            .global_remove(move |id| {
                if let Ok(mut state) = on_global_remove.try_borrow_mut() {
                    state.serials.remove(&id);
                }
            })
            .register();
        state.borrow_mut().connect(core.clone());
        Ok(Self { _registry_listener, _core_listener, _registry: registry, _core: core, broken })
    }

    pub fn broken(&self) -> bool {
        self.broken.get()
    }
}
