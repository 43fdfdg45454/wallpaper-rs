use crate::{Resolved, Rotation, Rules};
use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc};

#[derive(Debug, Clone, Copy, Default)]
pub struct Turn {
    pub live: bool,
    pub fresh: bool,
    pub start: bool,
    pub step: Option<bool>,
    pub retime: bool,
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

fn timed<Tz: TimeZone>(rules: &Rules, after: DateTime<Utc>, tz: &Tz) -> Option<DateTime<Utc>> {
    let mut times = rules.at.clone();
    times.sort();
    let today = after.with_timezone(tz).date_naive();
    days(rules, today).flat_map(|d| times.iter().filter_map(move |t| local(tz, d, *t))).find(|t| *t > after)
}

fn every<Tz: TimeZone>(rules: &Rules, from: DateTime<Utc>, tz: &Tz) -> Option<DateTime<Utc>> {
    let due = TimeDelta::from_std(rules.interval).ok().filter(|d| !d.is_zero()).map(|d| from + d)?;
    match days(rules, due.with_timezone(tz).date_naive()).next() {
        Some(day) if day == due.with_timezone(tz).date_naive() => Some(due),
        day => day.and_then(|day| local(tz, day, NaiveTime::MIN)),
    }
}

pub(crate) fn next_due<Tz: TimeZone>(rules: &Rules, after: DateTime<Utc>, tz: &Tz) -> Option<DateTime<Utc>> {
    timed(rules, after, tz).into_iter().chain(every(rules, after, tz)).min()
}

fn retimed<Tz: TimeZone>(
    rules: &Rules,
    (since, next_at): (Option<DateTime<Utc>>, Option<i64>),
    now: DateTime<Utc>,
    tz: &Tz,
) -> Option<i64> {
    let interval = TimeDelta::from_std(rules.interval).unwrap_or_default();
    let started = next_at.and_then(|at| DateTime::from_timestamp(at, 0)).map(|at| at - interval);
    let from = since.into_iter().chain(started).max().filter(|at| *at <= now).unwrap_or(now);
    timed(rules, now, tz).into_iter().chain(every(rules, from, tz)).min().map(|t| t.timestamp())
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
        row.state.restart(now);
        row.next_at = next_due(rules, now, tz).map(|t| t.timestamp());
    } else if turn.retime {
        row.next_at = retimed(rules, (row.state.since(), row.next_at), now, tz);
    }
    row.current = row.state.index(list).map(|i| uids[i].clone());
    step.is_some()
}

#[cfg(test)]
mod tests;
