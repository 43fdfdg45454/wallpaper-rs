use anyhow::{Context, Result, ensure};
use std::fs;
use std::os::unix::fs::{MetadataExt, lchown};
use std::path::Path;

const SERVICE: u32 = 65532;

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Stay,
    Drop { chown: bool },
}

pub fn decide(euid: u32, data_owner: Option<(u32, u32)>) -> Step {
    match euid {
        0 => Step::Drop { chown: data_owner != Some((SERVICE, SERVICE)) },
        _ => Step::Stay,
    }
}

pub fn settle(data: &Path) -> Result<()> {
    let owner = fs::metadata(data).ok().map(|meta| (meta.uid(), meta.gid()));
    let Step::Drop { chown } = decide(unsafe { libc::geteuid() }, owner) else { return Ok(()) };
    if chown {
        fs::create_dir_all(data).with_context(|| format!("cannot create {}", data.display()))?;
        give(data).with_context(|| format!("cannot hand {} to user {SERVICE}", data.display()))?;
    }
    let dropped = unsafe {
        libc::setgroups(0, std::ptr::null()) == 0 && libc::setgid(SERVICE) == 0 && libc::setuid(SERVICE) == 0
    };
    ensure!(dropped, "cannot switch to user {SERVICE}: the container needs CHOWN, SETUID and SETGID");
    Ok(())
}

fn give(path: &Path) -> Result<()> {
    lchown(path, Some(SERVICE), Some(SERVICE))?;
    if fs::symlink_metadata(path)?.is_dir() {
        for entry in fs::read_dir(path)? {
            give(&entry?.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_root_drops_and_only_when_the_owner_differs() {
        assert_eq!(decide(SERVICE, None), Step::Stay);
        assert_eq!(decide(1000, Some((0, 0))), Step::Stay);
        assert_eq!(decide(0, None), Step::Drop { chown: true });
        assert_eq!(decide(0, Some((0, 0))), Step::Drop { chown: true });
        assert_eq!(decide(0, Some((SERVICE, 0))), Step::Drop { chown: true });
        assert_eq!(decide(0, Some((SERVICE, SERVICE))), Step::Drop { chown: false });
    }
}
