use super::*;

#[test]
fn switches_follow_intervals_times_and_days() {
    let due = |r: &Rules, after: &str| next_due(r, at(after), &Utc).map(|t| t.to_rfc3339());
    let timed = rules(serde_json::json!({"interval": "0s", "at": ["18:00", "09:00"]}));
    assert_eq!(
        due(&rules(serde_json::json!({})), "2026-10-21T08:00:00Z").unwrap(),
        "2026-10-21T08:30:00+00:00"
    );
    assert_eq!(due(&timed, "2026-10-21T08:00:00Z").unwrap(), "2026-10-21T09:00:00+00:00");
    assert_eq!(due(&timed, "2026-10-21T09:00:00Z").unwrap(), "2026-10-21T18:00:00+00:00");
    assert_eq!(due(&timed, "2026-10-21T18:30:00Z").unwrap(), "2026-10-22T09:00:00+00:00");
    let monday = rules(serde_json::json!({"interval": "0s", "at": ["09:00"], "days": ["Mon"]}));
    assert_eq!(due(&monday, "2026-10-21T10:00:00Z").unwrap(), "2026-10-26T09:00:00+00:00");
    let hourly = rules(serde_json::json!({"interval": "1h", "days": ["mon"]}));
    assert_eq!(due(&hourly, "2026-10-24T12:00:00Z").unwrap(), "2026-10-26T00:00:00+00:00");
    assert_eq!(due(&rules(serde_json::json!({"interval": "0s"})), "2026-10-24T12:00:00Z"), None);
}

#[test]
fn skipped_times_fire_after_the_gap_and_repeated_ones_once() {
    let half = rules(serde_json::json!({"interval": "0s", "at": ["02:30"]}));
    let spring = Shift(at("2026-03-29T01:00:00Z").timestamp(), 3600, 7200);
    assert_eq!(next_due(&half, at("2026-03-29T00:00:00Z"), &spring), Some(at("2026-03-29T01:00:00Z")));
    let autumn = Shift(at("2026-10-25T01:00:00Z").timestamp(), 7200, 3600);
    assert_eq!(next_due(&half, at("2026-10-24T22:00:00Z"), &autumn), Some(at("2026-10-25T00:30:00Z")));
    assert_eq!(next_due(&half, at("2026-10-25T00:30:00Z"), &autumn), Some(at("2026-10-26T01:30:00Z")));
}

#[test]
fn rotation_waits_while_paused_and_advances_once() {
    let (resolved, rules, t0) =
        (list(&["a", "b", "c"], Order::Sequential), Rules::default(), at("2026-10-21T08:00:00Z"));
    let (mut row, live) = (row(), Turn { live: true, ..Turn::default() });
    let turn = |row: &mut Rotation, secs: i64, turn: Turn| {
        let switched =
            rotate(row, Some(&resolved.uids), &resolved, &rules, turn, t0 + TimeDelta::seconds(secs), &Utc);
        (switched, row.current.clone().unwrap(), row.next_at.unwrap() - t0.timestamp())
    };
    assert_eq!(turn(&mut row, 0, Turn { fresh: true, start: true, ..live }), (false, "a".into(), 1800));
    assert_eq!(turn(&mut row, 600, live), (false, "a".into(), 1800));
    assert_eq!(turn(&mut row, 1801, live), (true, "b".into(), 3601));
    assert_eq!(turn(&mut row, 4000, Turn::default()), (false, "b".into(), 3601));
    assert_eq!(turn(&mut row, 5000, live), (true, "c".into(), 6800));
    assert_eq!(turn(&mut row, 5100, Turn { step: Some(false), ..live }), (true, "b".into(), 6900));
    assert_eq!(turn(&mut row, 5200, Turn { step: Some(true), ..live }), (true, "c".into(), 7000));
    assert_eq!(turn(&mut row, 9000, Turn { start: true, ..live }), (false, "c".into(), 10800));
    let mut fresh = super::row();
    let resume = Rules { resume: false, ..Rules::default() };
    assert!(rotate(&mut fresh, None, &resolved, &resume, Turn { fresh: true, ..live }, t0, &Utc));
    assert_eq!(fresh.current.as_deref(), Some("b"));
    assert!(rotate(
        &mut fresh,
        Some(&resolved.uids),
        &resolved,
        &resume,
        Turn { start: true, ..live },
        t0,
        &Utc
    ));
    assert_eq!(fresh.current.as_deref(), Some("c"));
}
