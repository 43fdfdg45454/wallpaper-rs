use crate::ffi::get;
use std::time::{Duration, Instant};

const LINGER: Duration = Duration::from_secs(1);

fn others() -> Vec<u32> {
    let me = std::process::id();
    match unsafe { libc::getpgrp() } as u32 == me {
        true => wprs_web::members(me).into_iter().filter(|p| *p != me).collect(),
        false => Vec::new(),
    }
}

pub fn quit(code: i32) -> ! {
    let views: Vec<u32> = crate::state::VIEWS.with(|v| v.borrow().keys().copied().collect());
    views.into_iter().for_each(crate::run::close);
    let a = get();
    if let Some(engine) = crate::state::engine() {
        unsafe {
            (a.g_object_unref)(engine.session);
            (a.g_object_unref)(engine.context);
        }
    }
    let until = Instant::now() + LINGER;
    while !others().is_empty() && Instant::now() < until {
        unsafe { (a.g_main_context_iteration)(std::ptr::null_mut(), 0) };
        std::thread::sleep(Duration::from_millis(5));
    }
    for pid in others() {
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }
    std::process::exit(code)
}
