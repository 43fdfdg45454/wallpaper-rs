use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg, sendmsg, socketpair,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

pub const MAX_MESSAGE: usize = 192 << 10;
const MAX_FDS: usize = 4;

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
        if bytes.len() > MAX_MESSAGE {
            return Err(io::Error::other("message too large"));
        }
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        let fds = &fds[..fds.len().min(MAX_FDS)];
        if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(fds)) {
            return Err(io::Error::other("too many descriptors"));
        }
        let flags = SendFlags::NOSIGNAL | SendFlags::DONTWAIT;
        sendmsg(&self.0, &[IoSlice::new(&bytes)], &mut control, flags)?;
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
        let fds: Vec<OwnedFd> = control
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
        let message = serde_json::from_slice(&buffer[..got.bytes]).map_err(io::Error::other)?;
        Ok(Some((message, fds)))
    }
}

impl AsFd for Link {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
