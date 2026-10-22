use crate::{Error, Result};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug)]
pub enum Poll {
    Wait(Duration),
    Done(Value),
    Stop(Error),
}

pub fn poll(interval: &mut u64, answer: Result<Value>) -> Poll {
    match answer {
        Ok(token) => Poll::Done(token),
        Err(e) if e.code() == "slow_down" => {
            *interval += 5;
            Poll::Wait(Duration::from_secs(*interval))
        }
        Err(e) if e.code() == "authorization_pending" || matches!(e, Error::Network(_)) => {
            Poll::Wait(Duration::from_secs(*interval))
        }
        Err(e) => Poll::Stop(e),
    }
}
