use std::fs;
use tempfile::TempDir;
use wprs_library::{EntryEdit, Error, Item, Library, Meta, Resolved, Rules};

const NOW: i64 = 1_700_000_000;

fn three_items() -> (TempDir, Library, String, [Item; 3]) {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let root = dir.path().join("pictures");
    fs::create_dir_all(&root).unwrap();
    ["a.jpg", "b.png", "c.mp4"].iter().for_each(|name| fs::write(root.join(name), name).unwrap());
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW).unwrap();
    let items = library.search("sort:title", NOW, 0, 10).unwrap().try_into().unwrap();
    (dir, library, source, items)
}

fn titles(items: Vec<Item>) -> Vec<String> {
    items.into_iter().map(|item| item.title).collect()
}

fn resolved(library: &Library, list: &str, exclude: &[&str]) -> Vec<String> {
    let exclude = exclude.iter().map(|uid| uid.to_string()).collect();
    let Resolved { playlist, uids } = library.resolve(list, NOW, false, &exclude).unwrap();
    let items: Vec<Item> = uids.iter().map(|uid| library.item(uid).unwrap()).collect();
    assert_eq!(playlist.items, items.iter().map(|item| item.path.clone()).collect::<Vec<_>>());
    titles(items)
}

#[test]
fn many_item_edits_check_every_uid_before_changing_anything() {
    let (_dir, mut library, _, [a, b, c]) = three_items();
    library.set_rating_many(&[&a.uid, &b.uid], 4).unwrap();
    assert!(
        matches!(library.set_rating_many(&[&a.uid, "NOPE"], 1), Err(Error::NotFound("item", u)) if u == "NOPE")
    );
    assert!(matches!(library.set_rating_many(&[&a.uid], 6), Err(Error::Invalid("rating", _))));
    library.set_favorite_many(&[&a.uid, &c.uid], true).unwrap();
    library.retag(&[&a.uid, &b.uid], &["Sea", "night"], &[]).unwrap();
    library.retag(&[&b.uid], &["calm"], &["SEA"]).unwrap();
    assert!(matches!(library.retag(&[&c.uid, "NOPE"], &["lost"], &[]), Err(Error::NotFound(..))));
    assert!(matches!(library.retag(&[&c.uid], &["ok", " "], &[]), Err(Error::Invalid("tag", _))));
    let summary = |item: &Item| {
        let item = library.item(&item.uid).unwrap();
        (item.rating, item.favorite, item.tags)
    };
    assert_eq!(summary(&a), (4, true, vec!["night".into(), "Sea".into()]));
    assert_eq!(summary(&b), (4, false, vec!["calm".into(), "night".into()]));
    assert_eq!(summary(&c), (0, true, vec![]));
    assert_eq!(titles(library.search("sea", NOW, 0, 10).unwrap()), ["a"]);
}

#[test]
fn renaming_a_tag_merges_it_and_keeps_search_in_sync() {
    let (_dir, mut library, _, [a, b, c]) = three_items();
    library.retag(&[&a.uid, &c.uid], &["sea"], &[]).unwrap();
    library.retag(&[&b.uid, &c.uid], &["ocean"], &[]).unwrap();
    assert_eq!(library.rename_tag("SEA", "Ocean").unwrap(), 2);
    assert_eq!(library.tags().unwrap(), [("ocean".to_string(), 3)]);
    assert!(library.search("sea", NOW, 0, 10).unwrap().is_empty());
    assert_eq!(library.rename_tag("ocean", "Water").unwrap(), 3);
    assert_eq!(library.rename_tag("water", "WATER").unwrap(), 3);
    assert_eq!(library.tags().unwrap(), [("WATER".to_string(), 3)]);
    assert_eq!(titles(library.search("water", NOW, 0, 10).unwrap()).len(), 3);
    assert!(library.search("ocean", NOW, 0, 10).unwrap().is_empty());
    assert!(matches!(library.rename_tag("nothing", "x"), Err(Error::NotFound("tag", _))));
    assert!(matches!(library.rename_tag("water", " "), Err(Error::Invalid("tag", _))));
}

#[test]
fn removing_items_and_sources_returns_the_removed_rows() {
    let (_dir, mut library, source, [a, b, c]) = three_items();
    assert!(matches!(library.remove_items(&[&a.uid, "NOPE"]), Err(Error::NotFound(..))));
    assert_eq!(library.count("", NOW).unwrap(), 3);
    let removed = library.remove_items(&[&a.uid, &b.uid, &a.uid]).unwrap();
    assert_eq!(removed.iter().map(|item| &item.path).collect::<Vec<_>>(), [&a.path, &b.path]);
    let remaining = library.item(&c.uid).unwrap();
    assert_eq!(library.remove_source(&source).unwrap(), [remaining]);
    assert_eq!(library.count("", NOW).unwrap(), 0);
}

#[test]
fn entry_edits_apply_only_when_the_entries_hold_the_expected_items() {
    let (_dir, mut library, _, [a, b, c]) = three_items();
    let list = library.create_playlist("Zeta", None, &Rules::default()).unwrap();
    library.add_to_playlist(&list, &[&a.uid, &b.uid, &a.uid, &c.uid]).unwrap();
    library.edit_entries("zeta", EntryEdit::Remove(&[(2, &a.uid)])).unwrap();
    assert!(matches!(library.edit_entries(&list, EntryEdit::Remove(&[(0, &b.uid)])), Err(Error::Conflict)));
    library.edit_entries(&list, EntryEdit::Move { from: 2, to: 0, expected: &c.uid }).unwrap();
    let error =
        library.edit_entries(&list, EntryEdit::Move { from: 0, to: 3, expected: &c.uid }).unwrap_err();
    assert_eq!(error.to_string(), "invalid position: 3 is not below 3");
    let twice = [(1, a.uid.as_str()), (1, a.uid.as_str())];
    assert!(matches!(
        library.edit_entries(&list, EntryEdit::Remove(&twice)),
        Err(Error::Invalid("position", _))
    ));
    assert_eq!(titles(library.playlist_items(&list, NOW).unwrap()), ["c", "a", "b"]);

    let alpha = library.create_playlist("Alpha", None, &Rules::default()).unwrap();
    library.add_to_playlist(&alpha, &[&a.uid]).unwrap();
    library.create_playlist("Smart", Some("kind:image"), &Rules::default()).unwrap();
    let names: Vec<String> = library.item_playlists(&a.uid).unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["Alpha", "Zeta"]);
    assert!(matches!(library.item_playlists("NOPE"), Err(Error::NotFound("item", _))));

    assert!(matches!(library.remove_from_playlist_many(&list, &[&a.uid, "NOPE"]), Err(Error::NotFound(..))));
    assert!(matches!(
        library.remove_from_playlist_many("Smart", &[&a.uid]),
        Err(Error::Invalid("playlist", _))
    ));
    library.remove_from_playlist_many(&list, &[&a.uid, &c.uid]).unwrap();
    assert_eq!(titles(library.playlist_items(&list, NOW).unwrap()), ["b"]);
}

#[test]
fn resolving_leaves_out_excluded_broken_and_missing_items() {
    let (_dir, mut library, source, [a, b, c]) = three_items();
    let list = library.create_playlist("Zeta", None, &Rules::default()).unwrap();
    library.add_to_playlist(&list, &[&c.uid, &a.uid, &b.uid, &a.uid]).unwrap();
    assert_eq!(resolved(&library, &list, &[]), ["c", "a", "b", "a"]);
    assert_eq!(resolved(&library, &list, &[&a.uid]), ["c", "b"]);
    library.set_meta(&b.uid, &Meta { error: Some("unreadable".into()), ..Meta::default() }).unwrap();
    fs::remove_file(&c.path).unwrap();
    library.scan(&source, NOW).unwrap();
    assert_eq!(resolved(&library, &list, &[]), ["a", "a"]);
}
