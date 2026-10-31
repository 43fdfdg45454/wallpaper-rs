use crate::fetch::{kind, stalled};
use reqwest::StatusCode;
use reqwest::blocking::RequestBuilder;
use std::io::{self, Read};
use std::sync::mpsc::SyncSender;

pub(crate) const CHUNK: usize = 64 << 10;

pub(crate) enum Got {
    Status(StatusCode),
    Bytes(Vec<u8>),
    End,
    Failed(io::Error),
}

pub(crate) fn stream(request: RequestBuilder, max: u64, tx: &SyncSender<Got>) {
    let mut response = match request.send() {
        Ok(response) => response,
        Err(e) => return drop(tx.send(Got::Failed(io::Error::new(kind(&e), e.without_url())))),
    };
    if tx.send(Got::Status(response.status())).is_err() || !response.status().is_success() {
        return;
    }
    let mut left = max.saturating_add(1);
    while left > 0 {
        let mut buf = vec![0; left.min(CHUNK as u64) as usize];
        match response.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                buf.truncate(n);
                left -= n as u64;
                if tx.send(Got::Bytes(buf)).is_err() {
                    return;
                }
            }
            Err(e) => return drop(tx.send(Got::Failed(stalled(e)))),
        }
    }
    drop(tx.send(Got::End));
}
