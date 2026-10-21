use crate::Command;
use crate::state::State;
use pipewire as pw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

const TICK: Duration = Duration::from_millis(500);

pub fn run(
    rx: pw::channel::Receiver<Command>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), pw::Error> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let registry = core.get_registry_rc()?;
    let state = Rc::new(RefCell::new(State::default()));

    let (on_global, on_global_remove) = (state.clone(), state.clone());
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            if global.type_ == pw::types::ObjectType::Node
                && let Some(serial) = global.props.and_then(|p| p.get("object.serial")?.parse().ok())
                && let Ok(mut state) = on_global.try_borrow_mut()
            {
                state.serials.insert(global.id, serial);
                state.resolve_pending();
            }
        })
        .global_remove(move |id| {
            if let Ok(mut state) = on_global_remove.try_borrow_mut() {
                state.serials.remove(&id);
            }
        })
        .register();

    let (on_command, core_for_commands, weak_loop) = (state.clone(), core.clone(), mainloop.downgrade());
    let _receiver = rx.attach(mainloop.loop_(), move |command| match command {
        Command::Quit => {
            if let Some(mainloop) = weak_loop.upgrade() {
                mainloop.quit();
            }
        }
        Command::Ensure { output, width, height, reply } => {
            on_command.borrow_mut().ensure(&core_for_commands, output, width, height, reply)
        }
        Command::Frame { output, frame } => on_command.borrow_mut().show(&output, frame),
        Command::Remove { output } => on_command.borrow_mut().remove(&output),
    });

    let on_tick = state.clone();
    let timer = mainloop.loop_().add_timer(move |_| on_tick.borrow_mut().tick());
    timer.update_timer(Some(TICK), Some(TICK)).into_result().map_err(pw::Error::SpaError)?;

    let _ = ready.send(Ok(()));
    mainloop.run();
    state.borrow_mut().close_streams();
    Ok(())
}
