const MONTHS: [[&str; 12]; 2] = [
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ],
];
const SHORT_MONTHS: [&str; 12] =
    ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"];
const DAYS: [[&str; 7]; 2] = [
    ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"],
];
const SHORT_DAYS: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
const DAY: f64 = 86_400.0;

pub(crate) fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = ((doy - (153 * mp + 2) / 5 + 1) as u32, if mp < 10 { mp + 3 } else { mp - 9 } as u32);
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn weekday(days: i64) -> u32 {
    (days + 3).rem_euclid(7) as u32 + 1
}

fn week(days: i64) -> i64 {
    let thursday = days - i64::from(weekday(days)) + 4;
    (thursday - days_from(civil(thursday).0, 1, 1)) / 7 + 1
}

pub(crate) fn number(name: &str, local: f64, step: f64) -> (f32, f64) {
    let (days, s) = (local.div_euclid(DAY) as i64, local.rem_euclid(DAY));
    let (value, every) = match name {
        "day" => ((s - s % step) / DAY, step),
        "hour" => ((s / 3600.0).floor(), 3600.0),
        "minute" => ((s / 60.0).floor() % 60.0, 60.0),
        "second" => (s.floor() % 60.0, 1.0),
        "weekday" => (f64::from(weekday(days)), DAY),
        _ => (f64::from(civil(days).1), DAY),
    };
    (value as f32, every - s % every)
}

pub fn format(pattern: &str, local: f64, lang: &str, dates: bool) -> (String, Option<f64>) {
    let (days, s) = (local.div_euclid(DAY) as i64, local.rem_euclid(DAY) as u32);
    let ((y, mo, d), wd) = (civil(days), weekday(days) as usize - 1);
    let (h, mi, sec) = (s / 3600, s / 60 % 60, s % 60);
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    let es = usize::from(lang.split(['-', '_']).next() == Some("es"));
    let month = mo as usize - 1;
    let short_month =
        if es == 1 { SHORT_MONTHS[month].to_string() } else { MONTHS[0][month][..3].to_string() };
    let short_day = if es == 1 { SHORT_DAYS[wd].to_string() } else { DAYS[0][wd][..3].to_string() };
    let time = [
        ("HH", format!("{h:02}"), 3600.0),
        ("H", h.to_string(), 3600.0),
        ("hh", format!("{h12:02}"), 3600.0),
        ("h", h12.to_string(), 3600.0),
        ("mm", format!("{mi:02}"), 60.0),
        ("m", mi.to_string(), 60.0),
        ("ss", format!("{sec:02}"), 1.0),
        ("s", sec.to_string(), 1.0),
        ("a", if h < 12 { "AM" } else { "PM" }.to_string(), 3600.0),
    ];
    let date = [
        ("yyyy", format!("{y:04}"), DAY),
        ("yy", format!("{:02}", y.rem_euclid(100)), DAY),
        ("MMMM", MONTHS[es][month].to_string(), DAY),
        ("MMM", short_month, DAY),
        ("MM", format!("{mo:02}"), DAY),
        ("M", mo.to_string(), DAY),
        ("dd", format!("{d:02}"), DAY),
        ("d", d.to_string(), DAY),
        ("EEEE", DAYS[es][wd].to_string(), DAY),
        ("EEE", short_day, DAY),
        ("D", (days - days_from(y, 1, 1) + 1).to_string(), DAY),
        ("w", week(days).to_string(), DAY),
    ];
    let tokens: Vec<_> = if dates { date.iter().chain(&time).collect() } else { time.iter().collect() };
    let (mut out, mut rest, mut step) = (String::new(), pattern, None::<f64>);
    'outer: while let Some(c) = rest.chars().next() {
        if dates && c == '\'' {
            let tail = &rest[1..];
            let end = tail.find('\'').unwrap_or(tail.len());
            out.push_str(if end == 0 { "'" } else { &tail[..end] });
            rest = tail.get(end + 1..).unwrap_or_default();
            continue;
        }
        for (token, text, every) in &tokens {
            if let Some(tail) = rest.strip_prefix(token) {
                (out, rest, step) = (out + text, tail, Some(step.map_or(*every, |s| s.min(*every))));
                continue 'outer;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    let step = if dates { step } else { Some(if pattern.contains('s') { 1.0 } else { 60.0 }) };
    (out, step.map(|every| every - local.rem_euclid(every)))
}
