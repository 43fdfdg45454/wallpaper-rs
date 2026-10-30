use super::*;
use wprs_library::Error;

#[test]
fn sorting_and_paging() {
    let Fixture { library, .. } = fixture();
    assert_eq!(ordered(&library, "sort:title", 0, 10), ["café", "forest", "night", "ocean waves", "square"]);
    assert_eq!(ordered(&library, "sort:title-", 0, 2), ["square", "ocean waves"]);
    assert_eq!(ordered(&library, "sort:title", 1, 2), ["forest", "night"]);
    assert_eq!(ordered(&library, "sort:rating-", 0, 3), ["night", "café", "ocean waves"]);
    assert_eq!(ordered(&library, "sort:added-", 0, 1), ["forest"]);
    assert_eq!(ordered(&library, "sort:used-", 0, 1), ["forest"]);
    assert_eq!(ordered(&library, "", 0, 1), ["forest"]);
    let mut shuffled = ordered(&library, "sort:random", 0, 10);
    shuffled.sort();
    assert_eq!(shuffled, titles(&library, ""));
    assert_eq!(library.count("sort:title kind:video", NOW).unwrap(), 2);
}

#[test]
fn normalized_queries_name_manual_playlists_by_uid_and_fix_random_seeds() {
    let Fixture { mut library, .. } = fixture();
    let picks = library.playlist("Picks").unwrap().uid;
    let normalized =
        library.normalize_query("-in:\"picks\"  \"ocean waves\" Kind:VIDEO sort:Random-", 7).unwrap();
    assert_eq!(normalized, format!("-in:{picks} \"ocean waves\" kind:VIDEO sort:Random:7-"));
    assert_eq!(library.normalize_query(&normalized, 9).unwrap(), normalized);
    assert_eq!(
        library.normalize_query("in:Picks sort:random", 3).unwrap(),
        format!("in:{picks} sort:random:3")
    );
    assert_eq!(titles(&library, &format!("in:{picks}")), ["forest", "ocean waves"]);
    library.create_playlist("Sea", Some("tag:sea"), &Default::default()).unwrap();
    assert!(titles(&library, "in:Sea").is_empty());
    let error = |query: &str| library.normalize_query(query, 1).unwrap_err().to_string();
    assert_eq!(error("in:sea"), "invalid query: in: needs a manual playlist");
    assert_eq!(error("in:nothing"), "invalid query: no playlist named \"nothing\"");
    assert!(matches!(library.normalize_query("size:big", 1), Err(Error::Query(_))));
}

#[test]
fn seeded_random_order_is_stable_and_follows_the_item_ids() {
    let Fixture { library, .. } = fixture();
    let seeded = ordered(&library, "sort:random:1000000000", 0, 10);
    assert_eq!(seeded, ["forest", "ocean waves", "café", "square", "night"]);
    assert_eq!(ordered(&library, "sort:random:1000000000", 1, 2), seeded[1..3]);
    assert_eq!(ordered(&library, "sort:random:1000000000-", 0, 1), ["night"]);
    for query in ["sort:random:", "sort:random:x", "sort:random:4294967296", "sort:random:-1"] {
        assert!(matches!(library.search(query, NOW, 0, 10), Err(Error::Query(_))), "{query}");
    }
}

#[test]
fn items_resolve_from_uids_unique_suffixes_and_paths() {
    let Fixture { library, _dir: dir, .. } = fixture();
    let raw = Connection::open(dir.path().join("library.db")).unwrap();
    raw.execute_batch(
        "UPDATE items SET uid = '01AAAAAAAAAAAAAAAAAAXYZ123' WHERE title = 'café'; \
        UPDATE items SET uid = '01BBBBBBBBBBBBBBBBBBXYZ123' WHERE title = 'night'",
    )
    .unwrap();
    let night = "01BBBBBBBBBBBBBBBBBBXYZ123";
    for reference in [night, "01bbbbbbbbbbbbbbbbbbxyz123", "bxyz123", "BBXYZ123"] {
        assert_eq!(library.resolve_item(reference).unwrap(), night, "{reference}");
    }
    let error = library.resolve_item("xyz123").unwrap_err();
    assert_eq!(error.to_string(), "invalid item: \"xyz123\" matches 2 items; use more characters");
    assert!(matches!(library.resolve_item("xyz12"), Err(Error::Invalid("item", _))));
    assert!(matches!(library.resolve_item("ZZZZZZ"), Err(Error::NotFound("item", _))));
    let pictures = dir.path().join("pictures");
    assert_eq!(library.resolve_item(pictures.join("./night.webm").to_str().unwrap()).unwrap(), night);
    let square =
        library.resolve_item(fs::canonicalize(&pictures).unwrap().join("square.webp").to_str().unwrap());
    assert_eq!(library.item(&square.unwrap()).unwrap().title, "square");
    assert!(matches!(
        library.resolve_item(pictures.join("none.png").to_str().unwrap()),
        Err(Error::NotFound(..))
    ));
}
