use super::engine::{Device, Server};
use super::{at, view};
use crate::*;
use serde_json::json;

fn list(name: &str, entries: &[&str], query: Option<&str>) -> View {
    let mut list = view(&[("name", json!(name))]);
    list.extend(query.map(|q| ("query".to_owned(), json!(q))));
    let entry = |(n, item): (usize, &&str)| (format!("e:{item}-0"), json!({"i": item, "p": n as f64}));
    list.extend(entries.iter().enumerate().map(entry));
    list
}

#[tokio::test]
async fn a_change_that_failed_to_apply_is_held_and_never_pushed_back() {
    let server = Server::default();
    let mut a = Device::new("a", &[("lists/p1", list("Sea", &["x"], None))]);
    let mut b = Device::new("b", &[]);
    a.sync(&server, 10).await;
    b.sync(&server, 11).await;
    a.local.insert("lists/p1".into(), list("Ocean", &["x", "y"], None));
    a.sync(&server, 20).await;
    let (before, head) = (b.replica.bases.clone(), server.records.borrow()["lists/p1"].0);
    let now = at(30, "b");
    let synced = crate::sync(&&server, &mut b.replica, &b.local, |_| Rules::default(), &now).await.unwrap();
    assert_eq!(synced.applied.len(), 1);
    let key = "lists/p1".to_string();
    b.replica.held.insert(key.clone(), b.replica.bases[&key].clone());
    let old = before[&key].clone();
    let kept = Doc::stamp(Some(&old.1), b.local.get(&key), |_| false, &now).unwrap_or(old.1);
    b.replica.bases.insert(key.clone(), (old.0, kept));
    for ms in [40, 50] {
        let synced = b.sync(&server, ms).await;
        assert_eq!(server.records.borrow()["lists/p1"].0, head);
        if ms == 40 {
            assert_eq!(synced.applied, [(key.clone(), Some(list("Ocean", &["x", "y"], None)))]);
        }
    }
    assert!(b.replica.held.is_empty());
    assert_eq!(b.local[&key], a.local[&key]);
}

#[tokio::test]
async fn incompatible_list_edits_keep_both_sides() {
    let server = Server::default();
    let mut a = Device::new("a", &[("lists/p1", list("Sea", &["x"], None))]);
    let mut b = Device::new("b", &[]);
    a.sync(&server, 10).await;
    b.sync(&server, 11).await;
    a.local.remove("lists/p1");
    a.sync(&server, 20).await;
    b.local.insert("lists/p1".into(), list("Sea", &["x", "y"], None));
    let synced = b.sync(&server, 15).await;
    assert!(synced.copies.is_empty());
    a.sync(&server, 30).await;
    let both = list("Sea", &["x", "y"], None);
    assert_eq!((a.local.get("lists/p1"), b.local.get("lists/p1")), (Some(&both), Some(&both)));
    a.local.insert("lists/p1".into(), list("Sea", &[], Some("tag:sea")));
    a.sync(&server, 40).await;
    b.local.insert("lists/p1".into(), list("Sea", &["x", "y", "z"], None));
    let synced = b.sync(&server, 41).await;
    assert_eq!(synced.copies, [("lists/p1".to_string(), list("Sea", &[], Some("tag:sea")))]);
    a.sync(&server, 50).await;
    assert_eq!(a.local["lists/p1"], list("Sea", &["x", "y", "z"], None));
    let base = Some(list("Sea", &["x"], None));
    let renamed = list("Lake", &["x"], None);
    assert_eq!(clash(base.as_ref(), None, Some(&renamed)), None);
    assert_eq!(clash(base.as_ref(), Some(&list("Sea", &["x", "y"], None)), None), Some(Winner::Mine));
}
