pub use crate::outside::launch_checks_from_here;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io::{Read, Write};

pub const CHECK_SHADER: &str = "--wprs-check-shader";
pub const CHECK_MEMORY: u64 = 1 << 30;
pub const CHECK_CPU: u64 = 4;
pub(crate) const MOST_REQUEST: usize = 4 << 20;
pub(crate) const UNLIMITED: &str = "the checker could not limit itself: ";

#[derive(Serialize, Deserialize)]
pub(crate) struct Request {
    pub(crate) id: String,
    pub(crate) json: String,
    pub(crate) wgsl: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Verdict {
    pub(crate) hash: String,
    pub(crate) refused: Option<String>,
}

pub fn serve_check() {
    serve_check_with(|_| ());
}

#[doc(hidden)]
pub fn serve_check_with(before: fn(&str)) {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(CHECK_SHADER)) {
        return;
    }
    let mut limit = |default| args.next().and_then(|a| a.to_str()?.parse().ok()).unwrap_or(default);
    let (memory, cpu) = (limit(CHECK_MEMORY), limit(CHECK_CPU));
    if let Err(e) = confine(memory, cpu) {
        eprintln!("{UNLIMITED}{e}");
        std::process::exit(4)
    }
    std::process::exit(if answer(before).is_ok() { 0 } else { 3 })
}

fn confine(memory: u64, cpu: u64) -> std::io::Result<()> {
    unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) };
    let limits =
        [(libc::RLIMIT_AS, memory, memory), (libc::RLIMIT_CPU, cpu, cpu + 1), (libc::RLIMIT_CORE, 0, 0)];
    for (resource, soft, hard) in limits {
        let mut now = libc::rlimit { rlim_cur: 0, rlim_max: libc::RLIM_INFINITY };
        if unsafe { libc::getrlimit(resource, &mut now) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let hard = (hard as libc::rlim_t).min(now.rlim_max);
        let limit = libc::rlimit { rlim_cur: (soft as libc::rlim_t).min(hard), rlim_max: hard };
        if unsafe { libc::setrlimit(resource, &limit) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn answer(before: fn(&str)) -> std::io::Result<()> {
    let mut text = Vec::new();
    std::io::stdin().lock().take(MOST_REQUEST as u64).read_to_end(&mut text)?;
    let Request { id, json, wgsl } = serde_json::from_slice(&text)?;
    before(&id);
    let checked = crate::shader::roomy(|| crate::package::checked(&id, &json, &wgsl));
    let verdict = Verdict { hash: crate::verdicts::hash(&id, &json, &wgsl), refused: checked.err() };
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, &verdict)?;
    out.flush()
}
