use super::{at, doc, title, view};
use crate::*;
use serde_json::json;
use std::collections::BTreeMap;

const PLAIN: Rules =
    Rules { weak: |_| false, keep_absent: false, needs_strong: false, clash: |_, _, _| None };
const LIBRARY: Rules =
    Rules { weak: |key| key == "title", keep_absent: true, needs_strong: true, clash: |_, _, _| None };

#[test]
fn concurrent_edits_of_different_fields_both_survive() {
    let base = doc(&[("name", json!("Sea"), 1), ("query", json!("tag:sea"), 1)]);
    let mine = Doc::stamp(
        Some(&base),
        Some(&view(&[("name", json!("Ocean")), ("query", json!("tag:sea"))])),
        |_| false,
        &at(5, "a"),
    )
    .unwrap();
    let theirs =
        Doc::stamp(Some(&base), Some(&view(&[("name", json!("Sea"))])), |_| false, &at(4, "b")).unwrap();
    assert_eq!(mine.merge(&theirs).view(), Some(view(&[("name", json!("Ocean"))])));
    assert_eq!(mine.merge(&theirs), theirs.merge(&mine));
}

#[test]
fn the_later_clock_wins_and_deletions_lose_to_later_edits() {
    let base = doc(&[("name", json!("Sea"), 1)]);
    let renamed = Doc::stamp(Some(&base), Some(&title("Lake")), |_| false, &at(9, "b")).unwrap();
    let deleted = Doc::stamp(Some(&base), None, |_| false, &at(5, "a")).unwrap();
    assert_eq!(deleted.view(), None);
    assert_eq!(deleted.merge(&renamed).view(), Some(title("Lake")));
    let late = Doc::stamp(Some(&base), None, |_| false, &at(12, "a")).unwrap();
    assert_eq!(renamed.merge(&late).view(), None);
    assert_eq!(Doc::cleared().merge(&renamed).view(), Some(title("Lake")));
}

#[test]
fn unchanged_views_produce_nothing() {
    let base = doc(&[("rating", json!(3), 1)]);
    assert_eq!(Doc::stamp(Some(&base), Some(&view(&[("rating", json!(3))])), |_| false, &at(2, "a")), None);
    assert_eq!(Doc::stamp(None, None, |_| false, &at(2, "a")), None);
}

#[test]
fn library_items_without_user_data_stay_private() {
    let step = step(None, None, Some(&title("IMG_1")), LIBRARY, &at(5, "a"));
    assert_eq!(step, Step::default());
    let remote = (4, doc(&[("title", json!("Dawn"), 0), ("t:sky", json!(true), 3)]));
    let step = crate::step(None, Some(&remote), Some(&title("IMG_1")), LIBRARY, &at(5, "a"));
    assert_eq!(step.push, None);
    assert_eq!(step.apply, Some(Some(view(&[("title", json!("Dawn")), ("t:sky", json!(true))]))));
    let absent = crate::step(None, Some(&remote), None, LIBRARY, &at(5, "a"));
    assert_eq!((absent.apply, absent.push, absent.base), (None, None, Some(remote)));
}

#[test]
fn stale_records_from_the_server_are_refused() {
    let base = (9, doc(&[("name", json!("New"), 8)]));
    let old = (10, doc(&[("name", json!("Old"), 2)]));
    let step = step(Some(&base), Some(&old), Some(&view(&[("name", json!("New"))])), PLAIN, &at(20, "a"));
    assert_eq!(step.apply, None);
    assert_eq!(
        step.push.map(|(seq, doc)| (seq, doc.view())),
        Some((10, Some(view(&[("name", json!("New"))]))))
    );
}

#[test]
fn moved_entries_keep_the_positions_of_the_others() {
    let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let base: BTreeMap<String, f64> =
        [("a", 0.0), ("b", 1.0), ("c", 2.0), ("d", 3.0)].map(|(k, v)| (k.into(), v)).into();
    assert_eq!(positions(&ids(&["a", "c", "b", "d"]), &base), [0.0, 0.5, 1.0, 3.0]);
    assert_eq!(positions(&ids(&["x", "a", "y", "b"]), &base), [-1.0, 0.0, 0.5, 1.0]);
    assert_eq!(positions(&ids(&["b", "c", "z"]), &base), [1.0, 2.0, 3.0]);
    assert_eq!(positions(&ids(&["p", "q"]), &BTreeMap::new()), [0.0, 1.0]);
}

#[test]
fn tombstones_without_payload_are_undone() {
    let base = (4, doc(&[("name", json!("Sea"), 3)]));
    let local = view(&[("name", json!("Sea"))]);
    let step = step(Some(&base), Some(&(5, Doc::cleared())), Some(&local), PLAIN, &at(9, "a"));
    assert_eq!(step.apply, None);
    assert_eq!(step.push.map(|(seq, doc)| (seq, doc.view())), Some((5, Some(local))));
    let fresh = crate::step(None, Some(&(5, Doc::cleared())), None, PLAIN, &at(9, "a"));
    assert_eq!((fresh.apply, fresh.push), (None, None));
}

#[test]
fn empty_fields_are_no_edit() {
    let base = (4, doc(&[("name", json!("Sea"), 3)]));
    let local = view(&[("name", json!("Sea")), ("r:transition_ms", json!(null))]);
    assert_eq!(
        step(Some(&base), None, Some(&local), PLAIN, &at(9, "a")),
        Step { base: Some(base.clone()), ..Step::default() }
    );
    let same = step(Some(&base), Some(&base), Some(&local), PLAIN, &at(9, "a"));
    assert_eq!((same.push, same.apply), (None, None));
}

#[test]
fn edits_travel_in_parts_without_their_source_and_large_parts_keep_the_last_sent() {
    let edit: wprs_core::Edit = serde_json::from_value(json!({
        "version": 1, "source": {"name": "IMG_1.jpg"}, "fit": "fit",
        "outputs": {"Android": {"connector": "Android", "crop": [0.25, 0.0, 0.5, 1.0]}}
    }))
    .unwrap();
    let fields = edit_fields(Some(&edit), None);
    let android = json!({"connector": "Android", "crop": [0.25, 0.0, 0.5, 1.0]});
    assert_eq!(fields, view(&[("e:fit", json!("fit")), ("e:outputs/Android", android.clone())]));
    let whole = json!({"version": 1, "fit": "fit", "outputs": {"Android": android}});
    assert_eq!(edit_of(&fields), Some(whole));
    assert_eq!((edit_of(&title("IMG_1")), edit_fields(None, None)), (None, View::new()));
    let stroke = json!({"radius": 0.1, "hardness": 0.5, "points": vec![[0.5, 0.5]; 9000]});
    let masks = json!({"version": 1, "masks": {"b": {"type": "brush", "strokes": [stroke]}}});
    let huge: wprs_core::Edit = serde_json::from_value(masks).unwrap();
    let base = view(&[("e:masks", json!({}))]);
    assert_eq!(edit_fields(Some(&huge), Some(&base)).get("e:masks"), Some(&json!({})));
    assert_eq!(edit_fields(Some(&huge), None).get("e:masks"), None);
    let held = edit_held(&view(&[("e:span", json!({})), ("t:sky", json!(true))]), fields);
    assert_eq!(held.keys().collect::<Vec<_>>(), ["e:fit", "e:outputs/Android", "e:span"]);
    assert_eq!(held["e:span"], json!(null));
}

#[test]
fn edits_made_apart_before_the_first_sync_both_survive() {
    let base = (1, doc(&[("title", json!("Hills"), 1)]));
    let desk =
        view(&[("title", json!("Hills")), ("e:ops", json!([{"id": "ev", "type": "exposure", "ev": 1.0}]))]);
    let pushed = step(Some(&base), Some(&base), Some(&desk), LIBRARY, &at(5, "a")).push.unwrap();
    let phone = view(&[("title", json!("Hills")), ("e:parallax", json!({"enabled": true}))]);
    let both = step(Some(&base), Some(&pushed), Some(&phone), LIBRARY, &at(9, "b"));
    let merged = both.apply.flatten().unwrap();
    assert!(merged.contains_key("e:ops") && merged.contains_key("e:parallax"), "{merged:?}");
    let sent = both.push.unwrap().1.view().unwrap();
    assert_eq!(sent, merged);
}
