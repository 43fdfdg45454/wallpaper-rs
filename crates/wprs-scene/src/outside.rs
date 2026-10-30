use crate::helper::{CHECK_CPU, CHECK_MEMORY, CHECK_SHADER, MOST_REQUEST, Request, Verdict};
use crate::reap::{drained, lost, waited};
pub use launch::launch_checks_from_here;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

mod launch;

pub const CHECK_WALL: Duration = Duration::from_secs(10);
pub const UNCHECKED: &str = "its shader could not be checked: ";
const MOST_VERDICT: u64 = 4 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checker {
    pub program: PathBuf,
    pub memory: u64,
    pub cpu: u64,
    pub wall: Duration,
}

impl Default for Checker {
    fn default() -> Checker {
        let me = Path::new("/proc/self/exe");
        let program = match me.exists() {
            true => me.to_path_buf(),
            false => std::env::current_exe().unwrap_or_else(|_| me.to_path_buf()),
        };
        Checker { program, memory: CHECK_MEMORY, cpu: CHECK_CPU, wall: CHECK_WALL }
    }
}

static CHECKER: Mutex<Option<Checker>> = Mutex::new(None);
static RUNS: AtomicU64 = AtomicU64::new(0);

pub fn set_checker(checker: Option<Checker>) {
    *CHECKER.lock().unwrap_or_else(PoisonError::into_inner) = checker;
}

pub fn check_out_of_process(on: bool) {
    set_checker(on.then(Checker::default));
}

pub fn checker() -> Option<Checker> {
    CHECKER.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

pub fn helper_runs() -> u64 {
    RUNS.load(Relaxed)
}

fn failed(why: impl std::fmt::Display) -> String {
    format!("{UNCHECKED}{why}")
}

pub(crate) fn ask(checker: &Checker, request: &Request, hash: &str) -> Result<Option<String>, String> {
    let text = serde_json::to_vec(request).map_err(failed)?;
    if text.len() > MOST_REQUEST {
        return Err(format!("its descriptor and shader pass {} MiB", MOST_REQUEST >> 20));
    }
    RUNS.fetch_add(1, Relaxed);
    let mut command = Command::new(&checker.program);
    command.arg(CHECK_SHADER).arg(checker.memory.to_string()).arg(checker.cpu.to_string()).env_clear();
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = crate::outside::launch::spawn(command)
        .map_err(|e| failed(format!("the checker could not be started: {e}")))?;
    let (stdin, stdout, stderr) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
    let (ended, out, said) = std::thread::scope(|s| {
        s.spawn(move || stdin.map(|mut pipe| pipe.write_all(&text)));
        let out = s.spawn(move || drained(stdout, MOST_VERDICT));
        let said = s.spawn(move || drained(stderr, 4096));
        let ended = waited(&mut child, checker.wall);
        (ended, out.join().unwrap_or_default(), said.join().unwrap_or_default())
    });
    let status = ended.map_err(|e| failed(format!("the checker was lost: {e}")))?.ok_or_else(|| {
        failed(format!("the checker took longer than {} seconds", checker.wall.as_secs_f32()))
    })?;
    match serde_json::from_slice::<Verdict>(&out) {
        Ok(verdict) if status.success() && verdict.hash == hash => Ok(verdict.refused),
        Ok(_) if status.success() => Err(failed("the checker answered for other files")),
        _ => Err(failed(lost(status, &String::from_utf8_lossy(&said), checker.cpu))),
    }
}
