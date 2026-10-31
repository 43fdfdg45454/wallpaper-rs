use super::*;

#[test]
fn list_changes_keep_the_item_shown() {
    let (rules, now, live) = (Rules::default(), at("2026-10-21T08:00:00Z"), Turn::default());
    let (old, mut row) = (list(&["a", "b", "c"], Order::Sequential), row());
    row.current = Some("b".into());
    let grown = list(&["x", "a", "b", "c"], Order::Sequential);
    rotate(&mut row, Some(&old.uids), &grown, &rules, live, now, &Utc);
    assert_eq!((row.current.as_deref(), row.state.position()), (Some("b"), 2));
    let shrunk = list(&["x", "a", "c"], Order::Sequential);
    rotate(&mut row, Some(&grown.uids), &shrunk, &rules, live, now, &Utc);
    assert_eq!(row.current.as_deref(), Some("c"));
    let shuffled = list(&["a", "b", "c", "d", "e"], Order::Shuffle);
    let mut row = super::row();
    rotate(&mut row, None, &shuffled, &rules, live, now, &Utc);
    let shown = row.current.clone();
    let more = list(&["a", "b", "c", "d", "e", "f"], Order::Shuffle);
    rotate(&mut row, Some(&shuffled.uids), &more, &rules, live, now, &Utc);
    assert_eq!(row.current, shown);
    let mut seen = vec![row.current.clone().unwrap()];
    for _ in 1..6 {
        rotate(&mut row, Some(&more.uids), &more, &rules, Turn { step: Some(true), ..live }, now, &Utc);
        seen.push(row.current.clone().unwrap());
    }
    seen.sort();
    assert_eq!(seen, ["a", "b", "c", "d", "e", "f"]);
}

#[test]
fn clock_and_time_zone_changes_recompute_the_next_switch() {
    let (resolved, now) = (list(&["a", "b"], Order::Sequential), at("2026-10-21T05:00:00Z"));
    let retime = |rules: &Rules, next_at: &str, tz: FixedOffset| {
        let mut row = Rotation { next_at: Some(at(next_at).timestamp()), current: Some("a".into()), ..row() };
        let since = rules.interval.as_secs() as i64;
        row.state.restart(Utc.timestamp_opt(row.next_at.unwrap() - since.max(1800), 0).unwrap());
        let turn = Turn { live: true, retime: true, ..Turn::default() };
        let switched = rotate(&mut row, Some(&resolved.uids), &resolved, rules, turn, now, &tz);
        (switched, Utc.timestamp_opt(row.next_at.unwrap(), 0).unwrap().to_rfc3339())
    };
    let (east, west) = (FixedOffset::east_opt(3 * 3600).unwrap(), FixedOffset::west_opt(3 * 3600).unwrap());
    let nine = rules(serde_json::json!({"interval": "0s", "at": ["09:00"]}));
    assert_eq!(retime(&nine, "2026-10-21T09:00:00Z", east), (false, "2026-10-21T06:00:00+00:00".into()));
    assert_eq!(retime(&nine, "2026-10-21T09:00:00Z", west), (false, "2026-10-21T12:00:00+00:00".into()));
    let half = Rules::default();
    assert_eq!(retime(&half, "2026-10-21T05:10:00Z", east), (false, "2026-10-21T05:10:00+00:00".into()));
    assert_eq!(retime(&half, "2026-10-22T05:10:00Z", east), (false, "2026-10-21T05:30:00+00:00".into()));
    assert_eq!(retime(&half, "2026-10-21T04:50:00Z", east), (true, "2026-10-21T05:30:00+00:00".into()));
    let both = rules(serde_json::json!({"interval": "1h", "at": ["09:00"]}));
    let mut row = Rotation { current: Some("a".into()), ..row() };
    let live = Turn { live: true, ..Turn::default() };
    rotate(
        &mut row,
        Some(&resolved.uids),
        &resolved,
        &both,
        Turn { start: true, ..live },
        at("2026-10-21T08:30:00Z"),
        &Utc,
    );
    assert_eq!(row.next_at, Some(at("2026-10-21T09:00:00Z").timestamp()));
    let moved = Turn { retime: true, ..live };
    let switched =
        rotate(&mut row, Some(&resolved.uids), &resolved, &both, moved, at("2026-10-21T08:40:00Z"), &east);
    assert_eq!((switched, row.next_at), (false, Some(at("2026-10-21T09:30:00Z").timestamp())));
    let back =
        rotate(&mut row, Some(&resolved.uids), &resolved, &both, moved, at("2026-10-21T08:45:00Z"), &west);
    assert_eq!((back, row.next_at), (false, Some(at("2026-10-21T09:30:00Z").timestamp())));
}

#[test]
fn a_position_saved_before_its_last_start_does_not_switch_at_once() {
    let (resolved, now) = (list(&["a", "b"], Order::Sequential), at("2026-10-21T05:00:00Z"));
    let mut row = Rotation {
        next_at: Some(at("2026-10-21T05:20:00Z").timestamp()),
        current: Some("a".into()),
        ..row()
    };
    row.state.restart(at("2026-10-18T05:00:00Z"));
    let turn = Turn { live: true, retime: true, ..Turn::default() };
    let east = FixedOffset::east_opt(3 * 3600).unwrap();
    let switched = rotate(&mut row, Some(&resolved.uids), &resolved, &Rules::default(), turn, now, &east);
    assert_eq!((switched, row.next_at), (false, Some(at("2026-10-21T05:20:00Z").timestamp())));
}
