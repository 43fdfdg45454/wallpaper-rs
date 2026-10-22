use libc::{BPF_ABS, BPF_JEQ, BPF_JMP, BPF_K, BPF_LD, BPF_RET, BPF_W, sock_filter};
use std::io::{Error, Result};

#[cfg(target_arch = "x86_64")]
const ARCH: u32 = 0xc000_003e;
#[cfg(target_arch = "aarch64")]
const ARCH: u32 = 0xc000_00b7;

const ALLOWED: [libc::c_long; 22] = [
    libc::SYS_read,
    libc::SYS_write,
    libc::SYS_writev,
    libc::SYS_close,
    libc::SYS_mmap,
    libc::SYS_munmap,
    libc::SYS_mremap,
    libc::SYS_mprotect,
    libc::SYS_brk,
    libc::SYS_madvise,
    libc::SYS_futex,
    libc::SYS_rt_sigreturn,
    libc::SYS_rt_sigprocmask,
    libc::SYS_sigaltstack,
    libc::SYS_clock_gettime,
    libc::SYS_gettimeofday,
    libc::SYS_getrandom,
    libc::SYS_sched_yield,
    libc::SYS_getpid,
    libc::SYS_gettid,
    libc::SYS_exit,
    libc::SYS_exit_group,
];

fn check(result: libc::c_long) -> Result<()> {
    if result < 0 { Err(Error::last_os_error()) } else { Ok(()) }
}

fn op(code: u32, k: u32, jt: u8, jf: u8) -> sock_filter {
    sock_filter { code: code as u16, jt, jf, k }
}

pub fn enter() -> Result<()> {
    unsafe { libc::syscall(libc::SYS_close_range, 3, libc::c_uint::MAX, 0) };
    let limits = [
        (libc::RLIMIT_AS, 256 << 20),
        (libc::RLIMIT_NOFILE, 3),
        (libc::RLIMIT_NPROC, 0),
        (libc::RLIMIT_CORE, 0),
    ];
    for (resource, value) in limits {
        check(
            unsafe { libc::setrlimit(resource, &libc::rlimit { rlim_cur: value, rlim_max: value }) }.into(),
        )?;
    }
    check(unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) }.into())?;
    let (load, ret) = (BPF_LD | BPF_W | BPF_ABS, BPF_RET | BPF_K);
    let mut filter = vec![op(load, 4, 0, 0), op(BPF_JMP | BPF_JEQ | BPF_K, ARCH, 1, 0)];
    filter.extend([op(ret, libc::SECCOMP_RET_KILL_PROCESS, 0, 0), op(load, 0, 0, 0)]);
    for nr in ALLOWED {
        filter
            .extend([op(BPF_JMP | BPF_JEQ | BPF_K, nr as u32, 0, 1), op(ret, libc::SECCOMP_RET_ALLOW, 0, 0)]);
    }
    filter.push(op(ret, libc::SECCOMP_RET_ERRNO | libc::EPERM as u32, 0, 0));
    let program = libc::sock_fprog { len: filter.len() as u16, filter: filter.as_mut_ptr() };
    check(unsafe { libc::syscall(libc::SYS_seccomp, libc::SECCOMP_SET_MODE_FILTER, 0, &program) })
}
