use super::saved::Peer;
use super::server::pause;
use super::{block_on, sync_error};
use crate::error::{Result, failed};
use crate::jobs::Ctl;
use std::time::{Duration, Instant};
use wprs_sync::{Api, Letter, Mail};

pub fn inbox(api: &Api) -> Result<Vec<Letter>> {
    block_on(wprs_sync::inbox(api)).map_err(sync_error)
}

pub fn ack(api: &Api, id: i64) -> Result<()> {
    block_on(wprs_sync::ack(api, id)).map_err(sync_error)
}

pub fn post(api: &Api, to: &str, body: &[u8]) -> Result<()> {
    block_on(wprs_sync::post(api, to, body)).map_err(sync_error)
}

pub fn from(api: &Api, peer: &str) -> Result<Vec<Letter>> {
    block_on(wprs_sync::from(api, peer)).map_err(sync_error)
}

pub fn next(api: &Api, peer: &Peer, pair: &str, step: &str) -> Result<Option<Mail>> {
    block_on(wprs_sync::next(api, peer, pair, step)).map_err(|e| match e {
        wprs_sync::Error::Invalid(message) => failed(message),
        e => sync_error(e),
    })
}

pub fn wait(api: &Api, peer: &Peer, pair: &str, step: &str, ctl: &Ctl) -> Result<Mail> {
    let deadline = Instant::now() + Duration::from_secs(600);
    while Instant::now() < deadline {
        if let Some(mail) = next(api, peer, pair, step)? {
            return Ok(mail);
        }
        pause(ctl, Duration::from_secs(2))?;
    }
    Err(failed(format!("{} did not answer within 10 minutes; start again", peer.name)))
}
