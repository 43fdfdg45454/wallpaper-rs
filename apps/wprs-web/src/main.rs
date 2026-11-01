mod engine;
mod ffi;
mod formats;
mod frames;
mod gbm;
mod glib;
mod ring;
mod run;
mod scheme;
mod script;
mod state;
mod view;
mod watch;

use ffi::{Ptr, get};
use std::ffi::c_int;
use std::os::fd::{FromRawFd, OwnedFd};
use wprs_web::{FromWeb, Link, ToWeb};

const SOCKET: i32 = 3;
const UNSANDBOXED: &str = "Bubblewrap does not work";

fn version() -> Result<String, String> {
    let a = ffi::api()?;
    frames::patch()?;
    let (major, minor) = unsafe { ((a.webkit_get_major_version)(), (a.webkit_get_minor_version)()) };
    match (major, minor) >= (2, 54) {
        true => Ok(format!("{major}.{minor}")),
        false => Err(format!("WPE WebKit {major}.{minor} is older than 2.54")),
    }
}

unsafe extern "C" fn logged(level: c_int, fields: Ptr, count: usize, data: Ptr) -> c_int {
    let fields = unsafe { std::slice::from_raw_parts(fields.cast::<(ffi::Text, Ptr, isize)>(), count) };
    let message = fields.iter().find(|(key, ..)| unsafe { glib::string(*key) } == "MESSAGE");
    if message.is_some_and(|(_, value, _)| unsafe { glib::string(value.cast()) }.contains(UNSANDBOXED)) {
        state::send(&FromWeb::Said("sandbox: off".into()));
    }
    unsafe { (get().g_log_writer_standard_streams)(level, fields.as_ptr() as Ptr, count, data) }
}

unsafe extern "C" fn readable(_fd: c_int, _condition: u32, _data: Ptr) -> c_int {
    let Some(engine) = state::engine() else { return 0 };
    loop {
        match engine.link.recv::<ToWeb>(false) {
            Ok(Some((message, _))) => run::handle(message),
            Ok(None) => std::process::exit(0),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return 1,
            Err(_) => std::process::exit(1),
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    if let (Some(wprs_web::PROBE), Some(node)) = (args.next().as_deref(), args.next()) {
        std::process::exit(if gbm::allocates(&node) { 0 } else { 3 });
    }
    if std::env::args().nth(1).as_deref() == Some("--check") {
        match version() {
            Ok(version) => println!("{version}"),
            Err(reason) => {
                println!("{reason}");
                std::process::exit(2);
            }
        }
        return;
    }
    let link = Link::from_fd(unsafe { OwnedFd::from_raw_fd(SOCKET) });
    let start = match link.recv::<ToWeb>(true) {
        Ok(Some((ToWeb::Start(start), _))) => *start,
        _ => std::process::exit(1),
    };
    let version = match version() {
        Ok(version) => version,
        Err(reason) => {
            state::say(&link, &FromWeb::Unavailable(reason), &[]);
            std::process::exit(2);
        }
    };
    let a = get();
    unsafe {
        (a.g_log_set_writer_func)(logged as *const () as Ptr, std::ptr::null_mut(), std::ptr::null_mut())
    };
    state::live(|l| {
        (l.settings, l.allowed, l.properties) =
            (start.settings.clone(), start.allowed, start.properties.clone());
        l.files.extend(start.files.iter().cloned());
        for folder in &start.folders {
            l.files.extend(folder.files.iter().cloned());
            l.folders.insert(folder.prop.clone(), folder.clone());
        }
    });
    let raw = link.raw();
    if let Err(reason) = engine::start(link, start) {
        state::send(&FromWeb::Failed(reason));
        std::process::exit(1);
    }
    state::send(&FromWeb::Ready { version, sandboxed: true });
    unsafe {
        (a.g_unix_fd_add)(raw, 1 | 16, readable as *const () as Ptr, std::ptr::null_mut());
        (a.g_main_loop_run)((a.g_main_loop_new)(std::ptr::null_mut(), 0));
    }
}
