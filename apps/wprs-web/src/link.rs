use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, fcntl_get_seals, fstat, memfd_create};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg, sendmsg, socketpair,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{self, IoSlice, IoSliceMut, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

pub const MAX_MESSAGE: usize = 192 << 10;
pub const MAX_LARGE: u64 = 64 << 20;
const INLINE: usize = 32 << 10;
const MAX_FDS: usize = 4;
const LARGE: u8 = 0;

pub struct Link(OwnedFd, std::sync::Mutex<Vec<u8>>);

impl Link {
    pub fn pair() -> io::Result<(Link, OwnedFd)> {
        let (a, b) = socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, SocketFlags::CLOEXEC, None)?;
        Ok((Link::from_fd(a), b))
    }

    pub fn from_fd(fd: OwnedFd) -> Link {
        Link(fd, std::sync::Mutex::new(vec![0; MAX_MESSAGE]))
    }

    pub fn raw(&self) -> RawFd {
        self.0.as_raw_fd()
    }

    pub fn send<T: Serialize>(&self, message: &T, fds: &[BorrowedFd<'_>]) -> io::Result<()> {
        let bytes = serde_json::to_vec(message).map_err(io::Error::other)?;
        if bytes.len() <= INLINE {
            return self.packet(&bytes, fds);
        }
        if bytes.len() as u64 > MAX_LARGE {
            return Err(io::Error::other("message too large"));
        }
        let memfd = memfd_create("wprs-web-message", MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;
        std::fs::File::from(memfd.try_clone()?).write_all(&bytes)?;
        fcntl_add_seals(&memfd, SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL)?;
        let all: Vec<BorrowedFd<'_>> = fds.iter().copied().chain([memfd.as_fd()]).collect();
        self.packet(&[LARGE], &all)
    }

    fn packet(&self, bytes: &[u8], fds: &[BorrowedFd<'_>]) -> io::Result<()> {
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        let fds = &fds[..fds.len().min(MAX_FDS)];
        if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(fds)) {
            return Err(io::Error::other("too many descriptors"));
        }
        let flags = SendFlags::NOSIGNAL | SendFlags::DONTWAIT;
        sendmsg(&self.0, &[IoSlice::new(bytes)], &mut control, flags)?;
        Ok(())
    }

    pub fn recv<T: DeserializeOwned>(&self, wait: bool) -> io::Result<Option<(T, Vec<OwnedFd>)>> {
        let mut buffer = self.1.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let flags = RecvFlags::CMSG_CLOEXEC | if wait { RecvFlags::empty() } else { RecvFlags::DONTWAIT };
        let got = match recvmsg(&self.0, &mut [IoSliceMut::new(&mut buffer)], &mut control, flags) {
            Ok(got) => got,
            Err(rustix::io::Errno::AGAIN) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(e) => return Err(e.into()),
        };
        let mut fds: Vec<OwnedFd> = control
            .drain()
            .filter_map(|m| match m {
                RecvAncillaryMessage::ScmRights(fds) => Some(fds),
                _ => None,
            })
            .flatten()
            .collect();
        if got.bytes == 0 {
            return Ok(None);
        }
        if got.flags.contains(rustix::net::ReturnFlags::TRUNC) {
            return Err(io::Error::other("message truncated"));
        }
        let message = match buffer[..got.bytes] {
            [LARGE] => large(&fds.pop().ok_or_else(|| io::Error::other("no message"))?)?,
            ref bytes => serde_json::from_slice(bytes).map_err(io::Error::other)?,
        };
        Ok(Some((message, fds)))
    }
}

fn large<T: DeserializeOwned>(memfd: &OwnedFd) -> io::Result<T> {
    let sealed = fcntl_get_seals(memfd)?.contains(SealFlags::SHRINK | SealFlags::WRITE);
    let size = fstat(memfd)?.st_size as u64;
    if !sealed || size > MAX_LARGE {
        return Err(io::Error::other("a large message is not sealed or too large"));
    }
    let mut bytes = vec![0; size as usize];
    std::os::unix::fs::FileExt::read_exact_at(&std::fs::File::from(memfd.try_clone()?), &mut bytes, 0)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

impl AsFd for Link {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
