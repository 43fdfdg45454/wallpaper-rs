use std::io::Read;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

pub(crate) fn lost(status: ExitStatus, said: &str, cpu: u64) -> String {
    match status.signal() {
        _ if said.contains(crate::helper::UNLIMITED) => {
            said.lines().find(|l| l.contains(crate::helper::UNLIMITED)).unwrap_or(said).trim().into()
        }
        _ if said.contains("memory allocation of") => "the checker ran out of memory".into(),
        _ if said.contains("overflowed its stack") => "the checker ran out of stack".into(),
        Some(libc::SIGXCPU) => format!("the checker used more than {cpu} seconds of processor time"),
        Some(libc::SIGKILL) => "the checker was killed, out of memory or processor time".into(),
        Some(signal) => format!("the checker crashed with signal {signal}"),
        None => format!("the checker crashed ({status})"),
    }
}

pub(crate) fn drained(pipe: Option<impl Read>, most: u64) -> Vec<u8> {
    let mut kept = Vec::new();
    if let Some(mut pipe) = pipe {
        drop((&mut pipe).take(most).read_to_end(&mut kept));
        drop(std::io::copy(&mut pipe, &mut std::io::sink()));
    }
    kept
}

pub(crate) fn waited(child: &mut Child, wall: Duration) -> std::io::Result<Option<ExitStatus>> {
    let (start, mut nap) = (Instant::now(), Duration::from_millis(1));
    loop {
        match child.try_wait() {
            Ok(None) if start.elapsed() < wall => {
                std::thread::sleep(nap.min(wall.saturating_sub(start.elapsed())));
                nap = (nap * 2).min(Duration::from_millis(20));
            }
            Ok(None) => {
                drop(child.kill());
                return child.wait().map(|_| None);
            }
            Ok(Some(status)) => return Ok(Some(status)),
            Err(e) => {
                drop(child.kill());
                drop(child.wait());
                return Err(e);
            }
        }
    }
}
