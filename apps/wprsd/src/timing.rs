use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc};
use std::time::Duration;
use wprs_library::{Resolved, Rotation, Rules};

pub const CHUNK: Duration = Duration::from_secs(900);

#[derive(Debug, Clone, Copy, Default)]
pub struct Turn {
    pub live: bool,
    pub fresh: bool,
    pub start: bool,
    pub step: Option<bool>,
}

fn days(rules: &Rules, from: NaiveDate) -> impl Iterator<Item = NaiveDate> + '_ {
    let allowed = |day: &NaiveDate| rules.days.is_empty() || rules.days.contains(&day.weekday());
    (0..8).filter_map(move |n| from.checked_add_days(Days::new(n))).filter(allowed)
}

fn local<Tz: TimeZone>(tz: &Tz, day: NaiveDate, time: NaiveTime) -> Option<DateTime<Utc>> {
    let wall = day.and_time(time);
    (0..=180)
        .find_map(|m| tz.from_local_datetime(&(wall + TimeDelta::minutes(m))).earliest())
        .map(|t| t.to_utc())
}

pub fn next_due<Tz: TimeZone>(rules: &Rules, after: DateTime<Utc>, tz: &Tz) -> Option<DateTime<Utc>> {
    let mut times = rules.at.clone();
    times.sort();
    let today = after.with_timezone(tz).date_naive();
    let timed = days(rules, today)
        .flat_map(|d| times.iter().filter_map(move |t| local(tz, d, *t)))
        .find(|t| *t > after);
    let every = TimeDelta::from_std(rules.interval).ok().filter(|d| !d.is_zero()).map(|d| after + d);
    let every = every.and_then(|due| match days(rules, due.with_timezone(tz).date_naive()).next() {
        Some(day) if day == due.with_timezone(tz).date_naive() => Some(due),
        day => day.and_then(|day| local(tz, day, NaiveTime::MIN)),
    });
    timed.into_iter().chain(every).min()
}

pub fn nap(due_ms: Option<i64>, now_ms: i64) -> Option<Duration> {
    let max = CHUNK.as_millis() as i64;
    due_ms.map(|due| Duration::from_millis(due.saturating_sub(now_ms).clamp(0, max) as u64))
}

pub fn rotate<Tz: TimeZone>(
    row: &mut Rotation,
    last: Option<&[String]>,
    resolved: &Resolved,
    rules: &Rules,
    turn: Turn,
    now: DateTime<Utc>,
    tz: &Tz,
) -> bool {
    let (list, uids) = (&resolved.playlist, &resolved.uids);
    if uids.is_empty() {
        return false;
    }
    if let Some(current) = row.current.as_ref().filter(|_| last != Some(uids.as_slice())) {
        let before = last.and_then(|l| l.iter().position(|u| u == current));
        let found = uids.iter().position(|u| u == current);
        let index = found.unwrap_or(before.unwrap_or(row.state.position() as usize).min(uids.len() - 1));
        row.state.seek(list, index);
    }
    let due = row.next_at.is_some_and(|at| at <= now.timestamp());
    let begun = turn.fresh || turn.start;
    let step = turn.step.or(((begun && !rules.resume) || (due && turn.live && !turn.start)).then_some(true));
    match step {
        Some(true) => drop(row.state.advance(list, now)),
        Some(false) => drop(row.state.back(list, now)),
        None => {}
    }
    if step.is_some() || turn.start || row.next_at.is_none() {
        row.next_at = next_due(rules, now, tz).map(|t| t.timestamp());
    }
    row.current = row.state.index(list).map(|i| uids[i].clone());
    step.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, MappedLocalTime, NaiveDateTime};
    use wprs_schedule::{Order, Playlist, PlaylistState};

    #[derive(Clone, Copy)]
    struct Shift(i64, i32, i32);

    impl Shift {
        fn offset(&self, utc: i64) -> FixedOffset {
            FixedOffset::east_opt(if utc < self.0 { self.1 } else { self.2 }).unwrap()
        }
    }

    impl TimeZone for Shift {
        type Offset = FixedOffset;

        fn from_offset(_: &FixedOffset) -> Self {
            Shift(0, 0, 0)
        }

        fn offset_from_local_date(&self, _: &NaiveDate) -> MappedLocalTime<FixedOffset> {
            MappedLocalTime::None
        }

        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
            let mut offsets =
                [self.1.max(self.2), self.1.min(self.2)].map(|s| FixedOffset::east_opt(s).unwrap()).to_vec();
            offsets.dedup();
            offsets.retain(|o| self.offset((*local - *o).and_utc().timestamp()) == *o);
            match offsets[..] {
                [one] => MappedLocalTime::Single(one),
                [first, second] => MappedLocalTime::Ambiguous(first, second),
                _ => MappedLocalTime::None,
            }
        }

        fn offset_from_utc_date(&self, _: &NaiveDate) -> FixedOffset {
            self.offset(0)
        }

        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            self.offset(utc.and_utc().timestamp())
        }
    }

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn rules(json: serde_json::Value) -> Rules {
        serde_json::from_value(json).unwrap()
    }

    fn list(uids: &[&str], order: Order) -> Resolved {
        let items = uids.iter().map(|u| format!("/w/{u}.png").into()).collect();
        let playlist =
            Playlist { id: "P".into(), items, order, interval: Duration::from_secs(1800), per_output: true };
        Resolved { playlist, uids: uids.iter().map(|u| u.to_string()).collect() }
    }

    fn row() -> Rotation {
        Rotation {
            output: "DP-1".into(),
            playlist: "P".into(),
            state: PlaylistState::new(7),
            next_at: None,
            current: None,
        }
    }

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
            let switched = rotate(
                row,
                Some(&resolved.uids),
                &resolved,
                &rules,
                turn,
                t0 + TimeDelta::seconds(secs),
                &Utc,
            );
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
        let mut fresh = self::row();
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
        let mut row = self::row();
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
    fn naps_last_at_most_one_chunk() {
        assert_eq!(nap(Some(10_000), 0), Some(Duration::from_secs(10)));
        assert_eq!(nap(Some(i64::MAX), 0), Some(CHUNK));
        assert_eq!(nap(Some(0), 5_000), Some(Duration::ZERO));
        assert_eq!(nap(None, 0), None);
    }
}
