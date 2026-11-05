mod common;

use common::manifest;
use wprs_format::{Error, Kind};

fn fields(errors: Vec<Error>) -> Vec<String> {
    let field = |e: Error| match e {
        Error::Invalid { field, .. } => field,
        other => panic!("expected an invalid field, got {other:?}"),
    };
    errors.into_iter().map(field).collect()
}

#[test]
fn every_broken_field_of_a_manifest_is_reported_and_check_names_the_first() {
    let mut m = manifest(Kind::Scene);
    assert!(m.faults().is_empty());
    m.authors = vec![String::new()];
    m.license = "((".into();
    m.tags = vec!["x".repeat(40)];
    assert_eq!(fields(m.faults()), ["authors", "license", "tags"]);
    assert!(matches!(m.check(), Err(Error::Invalid { field, .. }) if field == "authors"));
}

#[test]
fn a_license_and_the_list_of_authors_have_a_length_limit() {
    let mut m = manifest(Kind::Scene);
    m.license = "a".repeat(257);
    m.authors = (0..65).map(|i| format!("a{i}")).collect();
    assert_eq!(fields(m.faults()), ["authors", "license"]);
    m.license = "a".repeat(256);
    m.authors.truncate(64);
    assert!(m.faults().is_empty(), "{:?}", m.faults());
}
