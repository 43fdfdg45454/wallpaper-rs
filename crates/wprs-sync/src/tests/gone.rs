use super::engine::{Device, Server};
use super::view;
use crate::*;
use serde_json::json;

const ITEM: &str = "library/0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";

fn record(server: &Server) -> Doc {
    server.records.borrow()[ITEM].1.clone()
}

#[tokio::test]
async fn deletions_reach_the_other_devices_and_never_come_back() {
    let server = Server::default();
    let item = view(&[("title", json!("Dawn")), ("rating", json!(4))]);
    let (mut a, mut b) =
        (Device::new("a", &[(ITEM, item.clone())]), Device::new("b", &[(ITEM, item.clone())]));
    a.sync_by(&server, 10, rules).await;
    b.sync_by(&server, 11, rules).await;
    let mut c = Device::new("c", &[]);
    c.sync_by(&server, 12, rules).await;
    assert_eq!(record(&server).view(), Some(item.clone()), "an item missing on one device stays");
    a.local.remove(ITEM);
    a.replica.gone.insert(ITEM.into(), 20);
    a.sync_by(&server, 20, rules).await;
    assert!(record(&server).view().is_none() && a.replica.gone.is_empty());
    let synced = b.sync_by(&server, 30, rules).await;
    assert_eq!(synced.applied, [(ITEM.to_string(), None)]);
    assert!(!b.local.contains_key(ITEM));
    for (device, ms) in [(&mut b, 40), (&mut a, 41), (&mut c, 42)] {
        device.sync_by(&server, ms, rules).await;
    }
    assert!(record(&server).view().is_none() && !c.local.contains_key(ITEM));
    let late = 20 + LIFETIME_MS + 1;
    a.sync_by(&server, late, rules).await;
    assert!(record(&server).is_cleared(), "old tombstones become server tombstones that the server purges");
    let seq = server.records.borrow()[ITEM].0;
    b.sync_by(&server, late + 10, rules).await;
    assert_eq!(server.records.borrow()[ITEM].0, seq, "a server tombstone is not answered with a record");
    b.local.insert(ITEM.into(), view(&[("title", json!("Dawn again"))]));
    b.sync_by(&server, late + 20, rules).await;
    assert_eq!(record(&server).view().unwrap()["title"], "Dawn again", "an item added again syncs again");
    a.sync_by(&server, late + 30, rules).await;
    assert!(!a.local.contains_key(ITEM) && record(&server).view().is_some());
}

struct Busy;

impl Remote for Busy {
    async fn pull(&self, since: i64) -> Result<Page> {
        Ok(Page { seq: since, more: false, last: since, changes: Vec::new() })
    }

    async fn push(&self, batch: &[(Key, i64, Doc)]) -> Result<Vec<Outcome>> {
        let other =
            |base: i64| Outcome::Conflict(Some((base + 1, super::doc(&[("name", json!("x"), base as u64)]))));
        Ok(batch.iter().map(|(_, base, _)| other(*base)).collect())
    }
}

#[tokio::test]
async fn changes_that_keep_conflicting_are_counted_as_left() {
    let local = [("playlists/p".to_string(), view(&[("name", json!("Mine"))]))].into();
    let now = Hlc(1_000, 0, "a".into());
    let synced = sync(&Busy, &mut Replica::default(), &local, |_| Rules::default(), &now).await.unwrap();
    assert_eq!(synced.left, 1);
}

#[test]
fn records_keep_fields_this_version_does_not_know() {
    let stored: Doc =
        serde_json::from_str(r#"{"f": {"title": ["Dawn", [5, 0, "a"]]}, "v": 2, "x": {"y": 1}}"#).unwrap();
    let merged = Doc::default().merge(&stored);
    let text = serde_json::to_value(&merged).unwrap();
    assert_eq!((&text["v"], &text["x"]["y"]), (&json!(2), &json!(1)));
}
