use super::*;

#[test]
fn rules_are_checked_against_the_library_rules() {
    let rules = set_rule(&Rules::default(), "at", "18:00, 07:30,18:00").unwrap();
    let rules = set_rule(&rules, "days", "sunday,Mon").unwrap();
    let json = serde_json::to_value(&rules).unwrap();
    assert_eq!((&json["at"], &json["days"]), (&json!(["07:30", "18:00"]), &json!(["Mon", "Sun"])));
    assert_eq!(set_rule(&rules, "interval", "1h30m").unwrap().interval.as_secs(), 5400);
    assert_eq!(set_rule(&rules, "transition_ms", "250").unwrap().transition_ms, Some(250));
    let bad = [("interval", "5s"), ("interval", "31d"), ("order", "random"), ("at", "25:00")];
    for (key, value) in
        bad.into_iter().chain([("synced", "yes"), ("transition_ms", "10001"), ("colour", "red")])
    {
        assert!(set_rule(&rules, key, value).is_err(), "{key}={value}");
    }
}
