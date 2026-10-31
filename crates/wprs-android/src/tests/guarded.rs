use crate::page::script::guarded;
use rquickjs::{Context, Runtime};

const ZONED: &str = include_str!("zoned.js");
const AWAY: &str = "America/Argentina/Buenos_Aires";
const HOURS: &str = "10800000";

fn seen(hide: bool, expression: &str) -> String {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    let guard = guarded(hide, hide, "null", "https://0000000000000000.wpak.invalid");
    let probe =
        format!("(() => {{ try {{ return String({expression}); }} catch (e) {{ return e.name; }} }})()");
    context.with(|ctx| {
        let seen = ctx.eval::<String, _>(format!("{ZONED}\n{guard}\n{probe}"));
        seen.unwrap_or_else(|_| panic!("{:?}", ctx.catch().as_exception().map(|e| (e.message(), e.stack()))))
    })
}

#[test]
fn hidden_zones_and_languages_stay_hidden_through_every_argument() {
    let cases = [
        ("Intl.DateTimeFormat().resolvedOptions().timeZone", AWAY, "UTC"),
        (
            "new Intl.DateTimeFormat(undefined, { timeZone: undefined }).resolvedOptions().timeZone",
            AWAY,
            "UTC",
        ),
        (
            "new Date(0).toLocaleString(undefined, { timeZone: undefined, timeZoneName: 'short' })",
            "es-AR America/Argentina/Buenos_Aires",
            "en-US UTC",
        ),
        (
            "new Temporal.Instant().toLocaleString([], { timeZone: undefined })",
            "es-AR America/Argentina/Buenos_Aires",
            "en-US UTC",
        ),
        (
            "new Intl.DateTimeFormat('ja-JP', { timeZone: 'Asia/Tokyo' }).resolvedOptions().timeZone",
            "Asia/Tokyo",
            "Asia/Tokyo",
        ),
        ("new Intl.NumberFormat([]).resolvedOptions().locale", "es-AR", "en-US"),
        ("new Intl.DateTimeFormat([]).resolvedOptions().locale", "es-AR", "en-US"),
        ("new Intl.NumberFormat(['zz']).resolvedOptions().locale", "es-AR", "en-US"),
        ("new Intl.NumberFormat({}).resolvedOptions().locale", "es-AR", "en-US"),
        ("new Intl.NumberFormat(true).resolvedOptions().locale", "es-AR", "en-US"),
        ("new Intl.NumberFormat([, ]).resolvedOptions().locale", "es-AR", "en-US"),
        ("new Intl.NumberFormat('ja-JP').resolvedOptions().locale", "ja-JP", "ja-JP"),
        ("(5).toLocaleString([])", "es-AR", "en-US"),
        ("new Intl.DateTimeFormat(null)", "TypeError", "TypeError"),
        ("new Date('2020-01-01T00:00').getTime() - Date.UTC(2020, 0, 1)", HOURS, "0"),
        ("Date.parse('2020-01-01T00:00:00.000') - Date.UTC(2020, 0, 1)", HOURS, "0"),
        ("Date.parse('01/01/2024 09:00:00') - Date.UTC(2024, 0, 1, 9)", HOURS, "0"),
        ("new Date(new String('2020-01-01T00:00')).getTime() - Date.UTC(2020, 0, 1)", HOURS, "0"),
        ("Date.parse('2020-01-01T00:00-03:00') - Date.UTC(2020, 0, 1, 3)", "0", "0"),
        ("Date.parse('2020-01-01T00:00Z') - Date.UTC(2020, 0, 1)", "0", "0"),
        ("Date.parse('2020-01-01') - Date.UTC(2020, 0, 1)", "0", "0"),
        ("new Date(new Date(5)).getTime()", "5", "5"),
        ("new Date(2020, 0).getTime() - Date.UTC(2020, 0)", HOURS, "0"),
        ("new Date(Date.UTC(2024, 0, 1, 12)).getHours()", "9", "12"),
        ("new Date(Date.UTC(2024, 0, 1, 2)).getYear()", "123", "124"),
        ("new Date(Date.UTC(2024, 0, 1, 12)).getTimezoneOffset()", "180", "0"),
        ("new Document().lastModified", "01/01/2024 09:00:00", "01/01/2024 12:00:00"),
        (
            "Object.getPrototypeOf(new Date()).constructor.parse('2020-01-01T00:00') - Date.UTC(2020, 0, 1)",
            HOURS,
            "0",
        ),
    ];
    for (expression, shown, hidden) in cases {
        assert_eq!(
            (seen(false, expression), seen(true, expression)),
            (shown.into(), hidden.into()),
            "{expression}"
        );
    }
    for expression in [
        "new Date({ valueOf: () => '2020-01-01T00:00' }).getTime() - Date.UTC(2020, 0, 1)",
        "new Date(Date.UTC(2024, 5, 1, 12)).setYear(99) - Date.UTC(1999, 5, 1, 12)",
        "new Date(NaN).setYear(99) - Date.UTC(1999, 0, 1)",
    ] {
        assert_eq!(seen(true, expression), "0", "{expression}");
    }
}
