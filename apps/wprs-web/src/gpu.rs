use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

pub const PROBE: &str = "--gpu";
const PATIENCE: Duration = Duration::from_secs(2);
static PROBED: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());

pub fn device(program: &Path, node: Option<String>) -> Option<String> {
    let node = node?;
    let known = |probed: &Vec<(String, bool)>| probed.iter().find(|(n, _)| *n == node).map(|(_, ok)| *ok);
    let cached = known(&PROBED.lock().unwrap_or_else(|e| e.into_inner()));
    let usable = cached.unwrap_or_else(|| {
        let usable = allocates(program, &node);
        PROBED.lock().unwrap_or_else(|e| e.into_inner()).push((node.clone(), usable));
        usable
    });
    usable.then_some(node)
}

fn allocates(program: &Path, node: &str) -> bool {
    let mut command = Command::new(program);
    command.arg(PROBE).arg(node).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else { return false };
    let pid = child.id() as libc::pid_t;
    let (tx, rx) = mpsc::channel();
    let waiter = std::thread::Builder::new().name("wprs-web-gpu".into()).spawn(move || {
        let _ = tx.send(child.wait().is_ok_and(|s| s.success()));
    });
    if waiter.is_err() {
        unsafe { libc::kill(pid, libc::SIGKILL) };
        return false;
    }
    rx.recv_timeout(PATIENCE).unwrap_or_else(|_| {
        unsafe { libc::kill(pid, libc::SIGKILL) };
        false
    })
}
