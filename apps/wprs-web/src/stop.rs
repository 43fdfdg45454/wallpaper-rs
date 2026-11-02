use std::process::Child;
use std::time::{Duration, Instant};

pub const GRACE: Duration = Duration::from_millis(1500);
pub const DRAIN: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(10);

pub fn members(group: u32) -> Vec<u32> {
    let Ok(dir) = std::fs::read_dir("/proc") else { return Vec::new() };
    let live = |entry: std::fs::DirEntry| {
        let pid = entry.file_name().to_str()?.parse::<u32>().ok()?;
        let stat = std::fs::read_to_string(entry.path().join("stat")).ok()?;
        let mut fields = stat.rsplit_once(')')?.1.split_whitespace();
        let (state, of) = (fields.next()?, fields.nth(1)?.parse::<u32>().ok()?);
        (state != "Z" && state != "X" && of == group).then_some(pid)
    };
    dir.flatten().filter_map(live).collect()
}

pub fn drain(group: u32, within: Duration) -> bool {
    let until = Instant::now() + within;
    while !members(group).is_empty() {
        if Instant::now() >= until {
            unsafe { libc::kill(-(group as i32), libc::SIGKILL) };
            return false;
        }
        std::thread::sleep(POLL);
    }
    true
}

pub fn end(mut child: Child, grace: Duration, within: Duration) -> bool {
    let (group, until) = (child.id(), Instant::now() + grace);
    let quit = loop {
        match child.try_wait() {
            Ok(None) if Instant::now() < until => std::thread::sleep(POLL),
            Ok(None) => break false,
            _ => break true,
        }
    };
    if !quit {
        let _ = child.kill();
    }
    let _ = child.wait();
    drain(group, within) && quit
}
