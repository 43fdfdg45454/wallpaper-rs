use wprs_format::{MAX_VOTED, voted};

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
    let many = vec!["org.example.dawn".to_owned(); MAX_VOTED + 1];
    assert!(voted(&many, Some(&server)).is_none());
    assert_eq!(voted(&many[..MAX_VOTED], Some(&server)).unwrap().len(), MAX_VOTED);
}
