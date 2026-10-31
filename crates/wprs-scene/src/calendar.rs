use std::fmt::Write;

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

const TIME: [(&str, f64); 9] = [
    ("HH", 3600.0),
    ("H", 3600.0),
    ("hh", 3600.0),
    ("h", 3600.0),
    ("mm", 60.0),
    ("m", 60.0),
    ("ss", 1.0),
    ("s", 1.0),
    ("a", 3600.0),
];
const DATE: [(&str, f64); 12] = [
    ("yyyy", DAY),
    ("yy", DAY),
    ("MMMM", DAY),
    ("MMM", DAY),
    ("MM", DAY),
    ("M", DAY),
    ("dd", DAY),
    ("d", DAY),
    ("EEEE", DAY),
    ("EEE", DAY),
    ("D", DAY),
    ("w", DAY),
];

pub fn format(pattern: &str, local: f64, lang: &str, dates: bool) -> (String, Option<f64>) {
    let (days, s) = (local.div_euclid(DAY) as i64, local.rem_euclid(DAY) as u32);
    let ((y, mo, d), wd) = (civil(days), weekday(days) as usize - 1);
    let (h, mi, sec) = (s / 3600, s / 60 % 60, s % 60);
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    let es = usize::from(lang.split(['-', '_']).next() == Some("es"));
    let month = mo as usize - 1;
    let put = |out: &mut String, token: &str| {
        let _ = match token {
            "HH" => write!(out, "{h:02}"),
            "H" => write!(out, "{h}"),
            "hh" => write!(out, "{h12:02}"),
            "h" => write!(out, "{h12}"),
            "mm" => write!(out, "{mi:02}"),
            "m" => write!(out, "{mi}"),
            "ss" => write!(out, "{sec:02}"),
            "s" => write!(out, "{sec}"),
            "a" => write!(out, "{}", if h < 12 { "AM" } else { "PM" }),
            "yyyy" => write!(out, "{y:04}"),
            "yy" => write!(out, "{:02}", y.rem_euclid(100)),
            "MMMM" => write!(out, "{}", MONTHS[es][month]),
            "MMM" => write!(out, "{}", if es == 1 { SHORT_MONTHS[month] } else { &MONTHS[0][month][..3] }),
            "MM" => write!(out, "{mo:02}"),
            "M" => write!(out, "{mo}"),
            "dd" => write!(out, "{d:02}"),
            "d" => write!(out, "{d}"),
            "EEEE" => write!(out, "{}", DAYS[es][wd]),
            "EEE" => write!(out, "{}", if es == 1 { SHORT_DAYS[wd] } else { &DAYS[0][wd][..3] }),
            "D" => write!(out, "{}", days - days_from(y, 1, 1) + 1),
            _ => write!(out, "{}", week(days)),
        };
    };
    let tokens = DATE.iter().filter(|_| dates).chain(&TIME);
    let (mut out, mut rest, mut step) = (String::with_capacity(pattern.len() * 2), pattern, None::<f64>);
    'outer: while let Some(c) = rest.chars().next() {
        if dates && c == '\'' {
            let tail = &rest[1..];
            let end = tail.find('\'').unwrap_or(tail.len());
            out.push_str(if end == 0 { "'" } else { &tail[..end] });
            rest = tail.get(end + 1..).unwrap_or_default();
            continue;
        }
        for (token, every) in tokens.clone() {
            if let Some(tail) = rest.strip_prefix(token) {
                put(&mut out, token);
                (rest, step) = (tail, Some(step.map_or(*every, |s| s.min(*every))));
                continue 'outer;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    let step = if dates { step } else { Some(if pattern.contains('s') { 1.0 } else { 60.0 }) };
    (out, step.map(|every| every - local.rem_euclid(every)))
}
