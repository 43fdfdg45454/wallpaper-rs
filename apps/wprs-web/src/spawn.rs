use crate::Link;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Isolation {
    pub network: bool,
    pub devices: bool,
}

fn write(path: &[u8], text: &[u8]) -> bool {
    let fd = unsafe { libc::open(path.as_ptr().cast(), libc::O_WRONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return false;
    }
    let written = unsafe { libc::write(fd, text.as_ptr().cast(), text.len()) };
    unsafe { libc::close(fd) };
    written == text.len() as isize
}

fn enter(isolation: Isolation, ids: (&[u8], &[u8])) {
    let mut flags = libc::CLONE_NEWUSER;
    flags |= if isolation.network { libc::CLONE_NEWNET } else { 0 };
    flags |= if isolation.devices { libc::CLONE_NEWNS } else { 0 };
    if flags == libc::CLONE_NEWUSER || unsafe { libc::unshare(flags) } != 0 {
        return;
    }
    let mapped = write(b"/proc/self/setgroups\0", b"deny")
        && write(b"/proc/self/uid_map\0", ids.0)
        && write(b"/proc/self/gid_map\0", ids.1);
    if mapped && isolation.devices {
        let none = std::ptr::null();
        let private = (libc::MS_REC | libc::MS_PRIVATE) as libc::c_ulong;
        unsafe { libc::mount(none, c"/".as_ptr(), none, private, none.cast()) };
        let tmpfs = c"tmpfs".as_ptr();
        unsafe {
            libc::mount(tmpfs, c"/dev/dri".as_ptr(), tmpfs, libc::MS_NOSUID as libc::c_ulong, none.cast())
        };
    }
}

pub fn spawn(
    program: &Path,
    isolation: Isolation,
    command: impl FnOnce(&mut Command),
) -> io::Result<(Link, Child)> {
    let (link, theirs) = Link::pair()?;
    let raw = theirs.as_raw_fd();
    let (uid, gid) = (unsafe { libc::getuid() }, unsafe { libc::getgid() });
    let (uids, gids) = (format!("{uid} {uid} 1"), format!("{gid} {gid} 1"));
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    command(&mut cmd);
    unsafe {
        cmd.pre_exec(move || {
            let moved = if raw == 3 { libc::fcntl(3, libc::F_SETFD, 0) } else { libc::dup2(raw, 3) };
            if moved < 0 {
                return Err(io::Error::last_os_error());
            }
            enter(isolation, (uids.as_bytes(), gids.as_bytes()));
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        })
    };
    let child = cmd.spawn()?;
    drop(theirs);
    Ok((link, child))
}

pub fn isolated(pid: u32) -> bool {
    let own = std::fs::read_link("/proc/self/ns/net").ok();
    let theirs = std::fs::read_link(format!("/proc/{pid}/ns/net")).ok();
    own.is_some() && theirs.is_some() && own != theirs
}
