use super::covers::{Cover, fetch};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Asked {
    track: u64,
    address: String,
    allowed: bool,
}

pub(super) type Arrival = (Asked, Option<Arc<Cover>>);

pub(super) struct Pictures {
    asked: Asked,
    loading: Option<JoinHandle<()>>,
    pub(super) cover: Option<Arc<Cover>>,
    pub(super) previous: Option<Arc<Cover>>,
    fetched: [bool; 2],
    sender: UnboundedSender<Arrival>,
}

fn remote(address: &str) -> bool {
    address.starts_with("http://") || address.starts_with("https://")
}

impl Pictures {
    pub(super) fn new(sender: UnboundedSender<Arrival>) -> Pictures {
        let (asked, cover, previous) = Default::default();
        Pictures { asked, loading: None, cover, previous, fetched: [false; 2], sender }
    }

    pub(super) fn ask(&mut self, track: u64, address: &str, remote_on: bool) {
        if !remote_on {
            self.forget_fetched();
        }
        let allowed = remote(address) && remote_on;
        let asked = Asked { track, address: address.to_owned(), allowed };
        if self.asked == asked {
            return;
        }
        self.asked = asked.clone();
        if let Some(task) = self.loading.take() {
            task.abort();
        }
        if address.is_empty() || (remote(address) && !allowed) {
            return self.shift(None, false);
        }
        let sender = self.sender.clone();
        self.loading = Some(tokio::spawn(async move {
            let cover = fetch(&asked.address, asked.allowed).await;
            let _ = sender.send((asked, cover));
        }));
    }

    pub(super) fn arrived(&mut self, (asked, cover): Arrival) {
        if self.asked == asked {
            self.loading = None;
            self.shift(cover, asked.allowed);
        }
    }

    fn forget_fetched(&mut self) {
        if self.fetched[0] {
            self.cover = None;
        }
        if self.fetched[1] {
            self.previous = None;
        }
        self.fetched = [false; 2];
    }

    fn shift(&mut self, cover: Option<Arc<Cover>>, fetched: bool) {
        let digest = |c: &Option<Arc<Cover>>| c.as_ref().map(|c| c.digest);
        if digest(&cover) == digest(&self.cover) {
            return;
        }
        if let Some(old) = self.cover.take() {
            (self.previous, self.fetched[1]) = (Some(old), self.fetched[0]);
        }
        self.fetched[0] = fetched && cover.is_some();
        self.cover = cover;
    }

    pub(super) fn serial(&self) -> u64 {
        self.cover.as_ref().map_or(0, |c| c.serial)
    }
}

impl Drop for Pictures {
    fn drop(&mut self) {
        if let Some(task) = self.loading.take() {
            task.abort();
        }
    }
}
