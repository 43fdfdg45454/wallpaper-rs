use crate::{
    COUNT, CUSTOM, DATA, Device, LEDS, MAGIC, MAX_DEVICES, MAX_PACKET, NAME, PROTOCOL, UPDATED, VERSION,
};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const ASKED: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0} is not a host and port")]
    Address(String),
    #[error("cannot reach the OpenRGB server at {0}")]
    Unreachable(String),
    #[error("the OpenRGB server stopped answering")]
    Lost,
    #[error("the OpenRGB server sent something unexpected")]
    Broken,
}

pub struct Client {
    stream: TcpStream,
    pub version: u32,
    pub changed: bool,
}

fn lost(e: std::io::Error) -> Error {
    match e.kind() {
        ErrorKind::InvalidData => Error::Broken,
        _ => Error::Lost,
    }
}

impl Client {
    pub fn connect(address: &str, timeout: Duration) -> Result<Client, Error> {
        let wrong = || Error::Address(address.into());
        let found = address.to_socket_addrs().map_err(|_| wrong())?;
        let stream = found.into_iter().find_map(|a| TcpStream::connect_timeout(&a, timeout).ok());
        let stream = stream.ok_or_else(|| Error::Unreachable(address.into()))?;
        stream.set_nodelay(true).map_err(lost)?;
        stream.set_write_timeout(Some(timeout)).map_err(lost)?;
        stream.set_read_timeout(Some(ASKED)).map_err(lost)?;
        let mut client = Client { stream, version: 0, changed: false };
        client.send(0, PROTOCOL, &VERSION.to_le_bytes())?;
        client.version = match client.reply(PROTOCOL) {
            Ok(data) => {
                data.get(..4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).min(VERSION)
            }
            Err(Error::Lost) => 0,
            Err(e) => return Err(e),
        };
        client.stream.set_read_timeout(Some(timeout)).map_err(lost)?;
        client.send(0, NAME, b"wallpaper-rs\0")?;
        Ok(client)
    }

    fn send(&mut self, device: u32, id: u32, data: &[u8]) -> Result<(), Error> {
        let mut packet = crate::header(device, id, data.len() as u32).to_vec();
        packet.extend_from_slice(data);
        self.stream.write_all(&packet).map_err(lost)
    }

    fn packet(&mut self) -> Result<(u32, Vec<u8>), Error> {
        let mut head = [0u8; 16];
        self.stream.read_exact(&mut head).map_err(lost)?;
        let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
        if &head[..4] != MAGIC || word(12) > MAX_PACKET {
            return Err(Error::Broken);
        }
        let mut data = vec![0; word(12) as usize];
        self.stream.read_exact(&mut data).map_err(lost)?;
        Ok((word(8), data))
    }

    fn reply(&mut self, id: u32) -> Result<Vec<u8>, Error> {
        for _ in 0..64 {
            match self.packet()? {
                (found, data) if found == id => return Ok(data),
                (UPDATED, _) => self.changed = true,
                _ => {}
            }
        }
        Err(Error::Broken)
    }

    pub fn devices(&mut self) -> Result<Vec<Device>, Error> {
        self.send(0, COUNT, &[])?;
        let data = self.reply(COUNT)?;
        let count =
            data.get(..4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or(Error::Broken)?;
        let asked = if self.version > 0 { self.version.to_le_bytes().to_vec() } else { Vec::new() };
        let mut devices = Vec::new();
        for index in 0..count.min(MAX_DEVICES) {
            self.send(index, DATA, &asked)?;
            let data = self.reply(DATA)?;
            devices.push(crate::controller(index, &data, self.version)?);
        }
        self.changed = false;
        Ok(devices)
    }

    pub fn direct(&mut self, device: u32) -> Result<(), Error> {
        self.send(device, CUSTOM, &[])
    }

    pub fn paint(&mut self, device: u32, colors: &[[u8; 3]]) -> Result<(), Error> {
        self.send(device, LEDS, &crate::leds(colors))
    }

    pub fn heard(&mut self) -> Result<bool, Error> {
        self.stream.set_nonblocking(true).map_err(lost)?;
        let mut head = [0u8; 16];
        let waiting = match self.stream.peek(&mut head) {
            Ok(0) => Err(Error::Lost),
            Ok(n) => Ok(n == 16),
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(lost(e)),
        };
        self.stream.set_nonblocking(false).map_err(lost)?;
        if waiting? && let (UPDATED, _) = self.packet()? {
            self.changed = true;
        }
        Ok(self.changed)
    }
}
