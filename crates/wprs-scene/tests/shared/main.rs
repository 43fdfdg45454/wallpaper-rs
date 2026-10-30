use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use wprs_scene::*;

mod exits;

pub(crate) static ONE: Mutex<()> = Mutex::new(());
const ROLE: &str = "WPRS_BOOK_ROLE";

pub(crate) fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(String::from).collect()
}

#[test]
fn child() {
    let Ok(role) = std::env::var(ROLE) else { return };
    let mut words = role.split(' ');
    let (what, path, key) = (words.next().unwrap(), words.next().unwrap(), words.next().unwrap());
    let key = u64::from_str_radix(key, 16).unwrap();
    keep_strikes(Path::new(path));
    match what {
        "block" => (0..STRIKES).for_each(|_| strike(&[key]).unwrap()),
        _ => {
            trying(key).unwrap();
            println!("ready");
            std::io::stdout().flush().unwrap();
            std::io::stdin().lines().for_each(drop);
            if what == "settle" {
                tried(key);
            }
        }
    }
}

pub(crate) fn spawn(role: &str, apart: bool) -> Child {
    let exe = std::env::current_exe().unwrap();
    let mut command = if apart { Command::new("unshare") } else { Command::new(&exe) };
    if apart {
        command.args(["-Urpf", "--mount-proc", "--kill-child"]).arg(&exe);
    }
    command.args(["--exact", "child", "--nocapture", "--test-threads=1"]).env(ROLE, role);
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    if !role.starts_with("block") {
        let mut out = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while out.read_line(&mut line).unwrap() > 0 && !line.contains("ready") {
            line.clear();
        }
        assert!(line.contains("ready"), "the child never started its trial");
        child.stdout = Some(out.into_inner());
    }
    child
}

#[test]
fn a_long_running_process_sees_effects_another_process_blocked() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stopped-effects");
    keep_strikes(&path);
    assert!(!struck(0x77));
    assert!(spawn(&format!("block {} 77", path.display()), false).wait().unwrap().success());
    assert!(struck(0x77) && trying(0x77).is_err(), "{:?}", lines(&path));
}
