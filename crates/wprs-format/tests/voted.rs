use wprs_format::{MAX_ASKED, MAX_VOTED, vote_queries, voted};

#[test]
fn only_packages_of_the_server_repository_are_voted_on() {
    let (server, other) = ("a".repeat(64), "b".repeat(64));
    let specs = vec![
        "org.example.dawn".to_owned(),
        format!("{server}/org.example.dusk"),
        format!("{other}/org.example.dawn"),
    ];
    let asked = voted(&specs, Some(&server)).unwrap();
    assert_eq!(asked, [("org.example.dawn", "org.example.dawn"), (specs[1].as_str(), "org.example.dusk")]);
    assert_eq!(voted(&specs, None).unwrap(), [("org.example.dawn", "org.example.dawn")]);
    assert_eq!(voted(&specs, Some("")).unwrap().len(), 1);
    assert!(voted(&[format!("{other}/org.example.dawn")], Some(&server)).unwrap().is_empty());
}

#[test]
fn bad_ids_and_too_many_are_refused() {
    let server = "a".repeat(64);
    for bad in ["Not An Id", "/org.example.dawn", "org.example.dawn/", "x/y/org.example.dawn", ""] {
        assert!(voted(&[bad.to_owned()], Some(&server)).is_none(), "{bad}");
    }
    assert!(voted(&[format!("{}/org.example.dawn", "c".repeat(129))], Some(&server)).is_none());
    let many = vec!["org.example.dawn".to_owned(); MAX_ASKED + 1];
    assert!(voted(&many, Some(&server)).is_none());
    assert_eq!(voted(&many[..MAX_ASKED], Some(&server)).unwrap().len(), MAX_ASKED);
}

#[test]
fn the_packages_of_the_server_get_counts_however_many_other_rows_come_first() {
    let (server, other) = ("a".repeat(64), "b".repeat(64));
    let mut specs: Vec<String> = (0..300).map(|i| format!("{other}/org.example.other{i}")).collect();
    specs.extend((0..150).map(|i| format!("{server}/org.example.ours{i}")));
    let asked = voted(&specs, Some(&server)).unwrap();
    assert_eq!(asked.len(), 150, "rows of the server after 300 others were dropped");
    let queries = vote_queries(&asked);
    assert_eq!(queries.len(), 2, "more than {MAX_VOTED} ids went in one request");
    let ids: Vec<&str> =
        queries.iter().flat_map(|q| q.trim_start_matches("/api/v1/votes?ids=").split(',')).collect();
    assert_eq!(ids.len(), 150);
    assert!(ids.iter().all(|id| id.starts_with("org.example.ours")), "{ids:?}");
    assert!(vote_queries(&[]).is_empty());
}
