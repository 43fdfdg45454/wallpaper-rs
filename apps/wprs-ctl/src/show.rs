use anyhow::{Result, bail};
use chrono::{Local, TimeZone};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, from_value};
use std::collections::BTreeMap;
use std::fmt::Display;
use wprs_client::parse;
use wprs_core::model::{
    Field, Header, InstalledPackage, Job, JobKind, JobReply, JobsReply, Repo, Schema, UploadOutput,
};

pub fn reply<T: DeserializeOwned>(json: String, raw: bool, human: impl FnOnce(T) -> String) -> Result<()> {
    print!("{}", if raw { self::raw(&json)? } else { human(parse(&json)?) });
    Ok(())
}

pub fn raw(json: &str) -> Result<String> {
    parse::<Header>(json)?;
    Ok(format!("{json}\n"))
}

pub fn either(value: &impl Serialize, json: bool, text: String) -> Result<()> {
    println!("{}", if json { serde_json::to_string(value)? } else { text });
    Ok(())
}

pub fn clean(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { '\u{fffd}' } else { c }).collect()
}

pub fn table(head: &[&str], rows: impl IntoIterator<Item = Vec<String>>) -> String {
    let rows = rows.into_iter().map(|row| row.iter().map(|cell| clean(cell)).collect());
    let rows: Vec<Vec<String>> =
        [head.iter().map(|h| h.to_string()).collect()].into_iter().chain(rows).collect();
    let width = |i: usize| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..head.len()).map(width).collect();
    let line = |row: &Vec<String>| {
        let cells: Vec<String> = row.iter().zip(&widths).map(|(cell, w)| format!("{cell:<w$}")).collect();
        cells.join("  ").trim_end().to_string() + "\n"
    };
    rows.iter().map(line).collect()
}

pub fn lines<K: Display>(pairs: impl IntoIterator<Item = (K, String)>) -> String {
    pairs.into_iter().map(|(key, value)| clean(format!("{key} = {value}").trim_end()) + "\n").collect()
}

pub fn pick(pairs: Vec<(&str, String)>, key: Option<&str>) -> Result<String> {
    let Some(key) = key else { return Ok(lines(pairs)) };
    match pairs.iter().find(|(k, _)| *k == key) {
        Some((_, value)) => Ok(clean(value) + "\n"),
        None => bail!("unknown key {key}; the keys are {}", list(pairs.iter().map(|(k, _)| *k))),
    }
}

pub fn list<T: ToString>(items: impl IntoIterator<Item = T>) -> String {
    items.into_iter().map(|item| item.to_string()).collect::<Vec<_>>().join(", ")
}

pub fn short(uid: &str) -> String {
    uid.chars().skip(uid.chars().count().saturating_sub(8)).collect()
}

pub fn prefix(id: &str) -> String {
    id.chars().take(8).collect()
}

pub fn text<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

pub fn time(at: i64) -> String {
    Local.timestamp_opt(at, 0).single().map_or(at.to_string(), |t| t.format("%Y-%m-%d %H:%M").to_string())
}

pub fn when(at: Option<i64>) -> String {
    at.map_or("-".into(), time)
}

pub fn bytes(size: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let step = (0..units.len()).take_while(|i| size >= 1 << (10 * i)).last().unwrap_or(0);
    match step {
        0 => format!("{size} B"),
        _ => format!("{:.1} {}", size as f64 / (1u64 << (10 * step)) as f64, units[step]),
    }
}

pub fn progress(done: u64, total: u64) -> String {
    match total {
        0 => format!("{done}"),
        _ => format!("{done}/{total} ({}%)", done.min(total) * 100 / total),
    }
}

pub fn schema(s: Schema) -> String {
    let section = |title: &str, body: String| format!("{title}\n{body}\n");
    let head = ["KEY", "KIND", "DEFAULT", "RANGE", "VALUES", "FLAGS"];
    let fields = |title: &str, fields: &[Field]| section(title, table(&head, fields.iter().map(field)));
    let mut out = fields("options", &s.options) + &fields("rules", &s.rules) + &fields("auto", &s.auto);
    out += &fields("preset", &s.preset);
    for p in &s.providers {
        let secret = match &p.secret {
            Some(secret) if secret.required => secret.name.clone(),
            Some(secret) => format!("{}, optional", secret.name),
            None => "none".into(),
        };
        let hosts = if p.hosts.is_empty() { "its server".into() } else { list(&p.hosts) };
        out += &fields(&format!("provider {} (secret: {secret}; contacts: {hosts})", p.kind), &p.options);
    }
    let keys = s.query.iter().map(|k| vec![k.key.clone(), k.hint.clone()]);
    out += &section("query", table(&["KEY", "MEANING"], keys));
    out + &lines([("thumb_sizes", list(&s.thumb_sizes)), ("fits", list(&s.fits))])
}

fn field(f: &Field) -> Vec<String> {
    let range = f.min.zip(f.max).map_or(String::new(), |(min, max)| format!("{min}..{max}"));
    let flags = [(f.required, "required"), (f.clearable, "clearable")].into_iter().filter(|(on, _)| *on);
    let default = if f.default.is_null() { "-".into() } else { f.default.to_string() };
    vec![f.key.clone(), text(&f.kind), default, range, f.values.join(","), list(flags.map(|(_, flag)| flag))]
}

pub fn jobs(reply: JobsReply) -> String {
    let rows = reply.jobs.into_iter().map(|j| {
        let (kind, state, done, started) =
            (text(&j.kind), text(&j.state), progress(j.done, j.total), time(j.started_at));
        vec![
            j.id.to_string(),
            kind,
            state,
            done,
            started,
            j.target,
            j.error.map_or(String::new(), |e| e.message),
        ]
    });
    table(&["JOB", "KIND", "STATE", "PROGRESS", "STARTED", "TARGET", "ERROR"], rows)
}

pub fn job(reply: JobReply) -> String {
    let j = reply.job;
    lines([
        ("job", j.id.to_string()),
        ("kind", text(&j.kind)),
        ("target", j.target),
        ("state", text(&j.state)),
        ("progress", progress(j.done, j.total)),
        ("started", time(j.started_at)),
        ("finished", when(j.finished_at)),
        ("error", j.error.map_or("-".into(), |e| format!("{} ({})", e.message, e.name))),
        ("output", j.output.map_or("-".into(), |o| o.to_string())),
    ])
}

pub fn summary(job: &Job) -> String {
    let head = format!("job {} {} {}", job.id, text(&job.kind), text(&job.state));
    let output = job.output.clone().unwrap_or_default();
    let detail = match job.kind {
        JobKind::RepoAdd => {
            from_value(output).ok().map(|r: Repo| format!("{} ({} packages)", r.name, r.packages))
        }
        JobKind::Install => {
            from_value(output).ok().map(|p: InstalledPackage| format!("{} {}", p.id, p.version))
        }
        JobKind::Upload => from_value(output).ok().map(|u: UploadOutput| {
            format!("{} {} {}", u.package.unwrap_or_default(), u.version.unwrap_or_default(), u.state)
        }),
        JobKind::Unknown => None,
        _ => Some(counts(&output)).filter(|counts| !counts.is_empty()),
    };
    clean(&detail.map_or(head.clone(), |detail| format!("{head}: {detail}")))
}

fn counts(output: &Value) -> String {
    let parts = match output.get("reports").and_then(Value::as_array) {
        Some(reports) => reports.iter().filter_map(Value::as_object).collect(),
        None => output.as_object().into_iter().collect::<Vec<_>>(),
    };
    let mut sums = BTreeMap::<&str, u64>::new();
    for (key, value) in parts.into_iter().flatten() {
        if let Some(n) = value.as_u64().or(value.as_array().map(|a| a.len() as u64)) {
            *sums.entry(key).or_default() += n;
        }
    }
    list(sums.into_iter().map(|(key, n)| format!("{n} {key}")))
}
