use super::{at, view};
use crate::engine::Replica;
use crate::*;
use serde_json::json;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Default)]
struct Server {
    head: RefCell<i64>,
    records: RefCell<BTreeMap<Key, (i64, Doc)>>,
    race: RefCell<Option<(Key, Doc)>>,
}

impl Server {
    fn write(&self, key: &Key, base: i64, doc: &Doc) -> Outcome {
        let mut records = self.records.borrow_mut();
        match records.get(key) {
            Some((seq, current)) if *seq != base => Outcome::Conflict(Some((*seq, current.clone()))),
            None if base != 0 => Outcome::Conflict(None),
            _ => {
                *self.head.borrow_mut() += 1;
                let seq = *self.head.borrow();
                records.insert(key.clone(), (seq, doc.clone()));
                Outcome::Ok(seq)
            }
        }
    }
}

impl Remote for &Server {
    async fn pull(&self, since: i64) -> Result<Page> {
        let records = self.records.borrow();
        let mut changes: Vec<Change> = records
            .iter()
            .filter(|(_, (seq, _))| *seq > since)
            .map(|(key, (seq, doc))| Change { key: key.clone(), seq: *seq, doc: doc.clone() })
            .collect();
        changes.sort_by_key(|c| c.seq);
        let last = changes.last().map_or(since, |c| c.seq);
        Ok(Page { seq: *self.head.borrow(), more: false, last, changes })
    }

    async fn push(&self, batch: &[(Key, i64, Doc)]) -> Result<Vec<Outcome>> {
        if let Some((key, doc)) = self.race.borrow_mut().take() {
            let seq = self.records.borrow().get(&key).map_or(0, |r| r.0);
            self.write(&key, seq, &doc);
        }
        Ok(batch.iter().map(|(key, base, doc)| self.write(key, *base, doc)).collect())
    }
}

fn rules(collection: &str) -> Rules {
    match collection {
        "settings" => Rules { weak: |_| true, ..Rules::default() },
        _ => Rules::default(),
    }
}

struct Device {
    replica: Replica,
    local: BTreeMap<Key, View>,
    clock: Clock,
    name: &'static str,
}

impl Device {
    fn new(name: &'static str, local: &[(&str, View)]) -> Device {
        let local = local.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        Device { replica: Replica::default(), local, clock: Clock::default(), name }
    }

    async fn sync(&mut self, server: &Server, ms: u64) {
        let now = self.clock.tick(ms, self.name);
        let applied = crate::sync(&server, &mut self.replica, &self.local, rules, &now).await.unwrap();
        for (key, view) in applied {
            match view {
                Some(view) => drop(self.local.insert(key, view)),
                None => drop(self.local.remove(&key)),
            }
        }
    }
}

#[tokio::test]
async fn two_devices_converge_through_conflicts() {
    let server = Server::default();
    let sea = view(&[("name", json!("Sea"))]);
    let mut a = Device::new(
        "a",
        &[("playlists/p1", sea.clone()), ("settings/pause", view(&[("pause.on_battery", json!(false))]))],
    );
    let mut b = Device::new("b", &[("settings/pause", view(&[("pause.on_battery", json!(true))]))]);
    a.sync(&server, 10).await;
    b.sync(&server, 11).await;
    assert_eq!(b.local, a.local);
    a.local.insert("playlists/p1".into(), view(&[("name", json!("Ocean"))]));
    b.local.insert("playlists/p1".into(), view(&[("name", json!("Sea")), ("query", json!("tag:sea"))]));
    a.sync(&server, 20).await;
    let racing =
        view(&[("name", json!("Ocean")), ("query", json!("tag:sea")), ("r:order", json!("shuffle"))]);
    let base = a.replica.bases["playlists/p1"].1.clone();
    *server.race.borrow_mut() = Some((
        "playlists/p1".into(),
        Doc::stamp(Some(&base), Some(&racing), |_| false, &at(25, "c")).unwrap(),
    ));
    b.sync(&server, 30).await;
    a.sync(&server, 40).await;
    assert_eq!(a.local, b.local);
    assert_eq!(a.local["playlists/p1"], racing);
    b.local.remove("playlists/p1");
    b.sync(&server, 50).await;
    a.sync(&server, 60).await;
    assert!(!a.local.contains_key("playlists/p1"));
    assert_eq!(server.records.borrow().len(), 2);
}
