use std::io;
use std::process::{Child, Command};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock, PoisonError};

type Ask = (Command, Sender<io::Result<Child>>);

static LAUNCHER: OnceLock<Mutex<Sender<Ask>>> = OnceLock::new();

pub fn launch_checks_from_here() {
    LAUNCHER.get_or_init(|| {
        let (tx, rx) = channel::<Ask>();
        let launcher = std::thread::Builder::new().name("wprs-checks".into());
        drop(launcher.spawn(move || rx.into_iter().for_each(|(mut c, back)| drop(back.send(c.spawn())))));
        Mutex::new(tx)
    });
}

pub(crate) fn spawn(mut command: Command) -> io::Result<Child> {
    let Some(launcher) = LAUNCHER.get() else { return command.spawn() };
    let (back, answer) = channel();
    match launcher.lock().unwrap_or_else(PoisonError::into_inner).send((command, back)) {
        Ok(()) => answer.recv().map_err(io::Error::other)?,
        Err(mut unsent) => unsent.0.0.spawn(),
    }
}
