use super::summary;
use crate::args::fixture;
use crate::diff::Edit;
use crate::grid::{Grid, compose, info, kind, name};
use wprs_core::model::*;

#[test]
fn filters_compose_one_query() {
    assert_eq!(
        compose(" harbour ", "image", true, "night sky"),
        "kind:image fav:yes tag:\"night sky\" harbour"
    );
    assert_eq!(compose("", "", false, "sea"), "tag:sea");
    assert_eq!(compose("", "", false, ""), "");
}

fn reload(grid: &mut Grid, uids: &[&str]) -> Vec<Edit> {
    let edits = grid.plan(uids.iter().map(|uid| summary(uid)).collect());
    edits.iter().for_each(|edit| grid.apply(edit));
    edits
}

#[test]
fn reloads_update_rows_in_place() {
    let mut grid = Grid { size: 256, ..Grid::default() };
    grid.apply(&Edit::Insert(0, vec![summary("A"), summary("B"), summary("C")]));
    assert_eq!(grid.wanted(0, 9), ["A", "B", "C"]);
    assert_eq!((grid.thumb("B", "/tmp/b.jpg"), grid.toggle(1, true)), (vec![1], vec![1]));
    let renamed = ItemSummary { title: "Renamed".into(), ..summary("A") };
    let edits = grid.plan(vec![summary("N"), summary("M"), summary("B"), renamed.clone()]);
    let expected = [
        Edit::Remove(2, 3),
        Edit::Insert(0, vec![summary("N"), summary("M")]),
        Edit::Move(3, 2),
        Edit::Set(3, Box::new(renamed)),
    ];
    assert_eq!(edits, expected);
    edits.iter().for_each(|edit| grid.apply(edit));
    let uids: Vec<_> = grid.rows.iter().map(|r| r.item.uid.as_str()).collect();
    assert_eq!((uids, grid.rows[3].item.title.as_str()), (vec!["N", "M", "B", "A"], "Renamed"));
    assert_eq!((grid.rows[2].thumb.as_deref(), grid.rows[2].selected), (Some("/tmp/b.jpg"), true));
    assert_eq!(reload(&mut grid, &["N", "M", "B", "A"]), [Edit::Set(3, Box::new(summary("A")))]);
    assert!(reload(&mut grid, &["N", "M", "B", "A"]).is_empty());
    let twice =
        [Edit::Remove(3, 4), Edit::Remove(1, 2), Edit::Move(1, 0), Edit::Insert(1, vec![summary("B")])];
    assert_eq!(reload(&mut grid, &["B", "B", "N"]), twice);
    assert_eq!(reload(&mut grid, &["B"]), [Edit::Remove(2, 3), Edit::Remove(1, 2)]);
    assert_eq!((grid.rows.len(), grid.rows[0].selected), (1, true));
}

#[test]
fn changed_items_refresh_their_thumbnails() {
    let mut grid = Grid { size: 256, ..Grid::default() };
    grid.apply(&Edit::Insert(0, vec![summary("A"), summary("B")]));
    grid.thumb("A", "/tmp/a.jpg");
    grid.thumb("B", "/tmp/b.jpg");
    assert!(grid.wanted(0, 9).is_empty());
    grid.stale(&["B".into()]);
    assert_eq!(grid.wanted(0, 9), ["B"]);
    assert_eq!(grid.thumb("B", "/tmp/b.jpg"), Vec::<usize>::new());
    grid.stale(&[]);
    assert_eq!(grid.wanted(0, 9), ["A", "B"]);
    assert_eq!(grid.thumb("A", "/tmp/a2.jpg"), [0]);
    let cached = ItemSummary { thumb: Some("/tmp/b3.jpg".into()), ..summary("B") };
    grid.apply(&Edit::Set(1, Box::new(cached)));
    assert_eq!(grid.rows[1].thumb.as_deref(), Some("/tmp/b3.jpg"));
    assert!(grid.wanted(0, 9).is_empty());
    assert_eq!(grid.toggle(usize::MAX, true), Vec::<usize>::new());
}

#[test]
fn rows_describe_items() {
    let video = ItemSummary { kind: ItemKind::Video, duration_ms: Some(65000), ..summary("A") };
    assert_eq!(info(&video), "3840×2160 · 1:05");
    assert_eq!((name(ItemKind::Package), name(Reason::Takedown)), ("package".into(), "takedown".into()));
    let page: SearchPage = wprs_client::parse(&fixture("Library1", "Search")).unwrap();
    assert_eq!(
        page.items.iter().map(kind).collect::<Vec<_>>(),
        ["image", "video", "package", "image", "scene"]
    );
}

#[test]
fn a_change_asks_again_only_for_its_rows() {
    let mut grid = Grid { size: 256, ..Grid::default() };
    let uids = ["A", "B", "C", "D"];
    grid.apply(&Edit::Insert(0, uids.iter().map(|uid| summary(uid)).collect()));
    grid.toggle(2, true);
    let rated = ItemSummary { rating: 1, ..summary("C") };
    let changed = ["B".to_owned(), "C".to_owned(), "Z".to_owned()];
    let edits = grid.patch(&changed, vec![rated.clone()]).unwrap();
    assert_eq!(edits, [Edit::Set(2, Box::new(rated)), Edit::Remove(1, 2)]);
    edits.iter().for_each(|edit| grid.apply(edit));
    let left: Vec<_> = grid.rows.iter().map(|r| (r.item.uid.as_str(), r.selected)).collect();
    assert_eq!(left, [("A", false), ("C", true), ("D", false)]);
    assert_eq!(grid.patch(&["A".to_owned()], vec![summary("A")]).unwrap(), []);
    assert_eq!(grid.patch(&["N".to_owned()], vec![summary("N")]), None, "a new row needs its place");
    assert!(crate::diff::steady("tag:sea sort:added- sort:random:7") && crate::diff::steady("harbour"));
    assert!(!crate::diff::steady("sort:title") && !crate::diff::steady("fav:yes SORT:rating-"));
}
