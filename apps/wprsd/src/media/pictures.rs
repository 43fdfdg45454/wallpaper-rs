use super::covers::{Cover, fetch};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

pub(super) type Arrival = (String, bool, Option<Arc<Cover>>);

pub(super) struct Pictures {
    asked: (String, bool),
    loading: Option<JoinHandle<()>>,
    pub(super) cover: Option<Arc<Cover>>,
    pub(super) previous: Option<Arc<Cover>>,
    sender: UnboundedSender<Arrival>,
}

fn remote(address: &str) -> bool {
    address.starts_with("http://") || address.starts_with("https://")
}

impl Pictures {
    pub(super) fn new(sender: UnboundedSender<Arrival>) -> Pictures {
        Pictures { asked: Default::default(), loading: None, cover: None, previous: None, sender }
    }

    pub(super) fn ask(&mut self, address: &str) {
        let allowed = remote(address) && super::remote_on();
        if (self.asked.0.as_str(), self.asked.1) == (address, allowed) {
            return;
        }
        self.asked = (address.to_owned(), allowed);
        if let Some(task) = self.loading.take() {
            task.abort();
        }
        if address.is_empty() || (remote(address) && !allowed) {
            return self.shift(None);
        }
        let (sender, address) = (self.sender.clone(), address.to_owned());
        self.loading = Some(tokio::spawn(async move {
            let cover = fetch(&address, allowed).await;
            let _ = sender.send((address, allowed, cover));
        }));
    }

    pub(super) fn arrived(&mut self, (address, allowed, cover): Arrival) {
        if self.asked == (address, allowed) {
            self.loading = None;
            self.shift(cover);
        }
    }

    fn shift(&mut self, cover: Option<Arc<Cover>>) {
        let serial = |c: &Option<Arc<Cover>>| c.as_ref().map(|c| c.serial);
        if serial(&cover) == serial(&self.cover) {
            return;
        }
        if let Some(old) = self.cover.take() {
            self.previous = Some(old);
        }
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
