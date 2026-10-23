mod common;

use common::v2;
use std::collections::BTreeMap;
use wprs_format::{Control, Error, Kind, Manifest, Property, Text, markup};

fn map(pairs: &[(&str, &str)]) -> Text {
    Text::Localized(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

fn localized() -> Manifest {
    let mut m = v2(Kind::Scene);
    m.default_locale = Some("en".into());
    m.title = map(&[("en", "Dusk"), ("es", "Atardecer"), ("zh-Hans", "黄昏")]);
    m.description = "@about".into();
    m.changes = Some("@changes".into());
    let table = |about: &str| {
        BTreeMap::from([("about".to_string(), about.to_string()), ("speed".into(), "Speed".into())])
    };
    m.strings.insert("en".into(), table("Hills at **dusk**."));
    m.strings.insert("pt-BR".into(), table("Colinas ao *anoitecer*."));
    m.strings.get_mut("en").unwrap().insert("changes".into(), "- Faster\n- Smaller".into());
    let mut speed = Property::new(Control::Bool { default: true });
    speed.label = Some("@speed".into());
    m.properties.insert("speed".into(), speed);
    m
}

fn field(m: &Manifest) -> String {
    match m.check() {
        Err(Error::Invalid { field, .. }) => field,
        other => panic!("expected an invalid field, got {other:?}"),
    }
}

#[test]
fn texts_follow_the_language_chain() {
    let m = localized();
    m.check().unwrap();
    let pick = |langs: &[&str]| m.title_in(&langs.iter().map(|l| l.to_string()).collect::<Vec<_>>());
    assert_eq!(pick(&["es_ES.UTF-8"]), "Atardecer");
    assert_eq!(pick(&["es"]), "Atardecer");
    assert_eq!(pick(&["zh_CN"]), "黄昏");
    assert_eq!(pick(&["zh-chs"]), "黄昏");
    assert_eq!(pick(&["de_DE"]), "Dusk");
    let about = |l: &str| m.pick(&m.description, &[l.to_string()]);
    assert_eq!(
        (about("pt_BR"), about("pt"), about("fr")),
        ("Colinas ao *anoitecer*.".into(), "Colinas ao *anoitecer*.".into(), "Hills at **dusk**.".into())
    );
    let label = m.localized().properties["speed"].label.clone().unwrap();
    assert_eq!(label, map(&[("en", "Speed"), ("pt-BR", "Speed")]));
    assert_eq!(m.locales().into_iter().collect::<Vec<_>>(), ["en", "es", "pt-br", "zh-hans"]);
}

#[test]
fn broken_texts_are_rejected() {
    let mut m = localized();
    m.default_locale = None;
    assert_eq!(field(&m), "default_locale");
    let mut m = localized();
    m.description = "@missing".into();
    assert_eq!(field(&m), "description");
    let mut m = localized();
    m.title = map(&[("es", "Atardecer")]);
    assert_eq!(field(&m), "title");
    let mut m = localized();
    m.strings.insert("not a tag".into(), BTreeMap::new());
    assert_eq!(field(&m), "strings");
    for bad in ["two\nlines", "\u{feff}Dusk", &"t".repeat(129), " "] {
        let mut m = localized();
        m.title = map(&[("en", bad)]);
        assert_eq!(field(&m), "title", "{bad:?}");
    }
    let mut m = localized();
    m.strings.get_mut("en").unwrap().insert("about".into(), "<b>bold</b>".into());
    assert_eq!(field(&m), "description");
    let mut m = localized();
    m.title = map(&[("en", "Dusk"), ("es", "")]);
    assert_eq!(m.check().unwrap_err().to_string(), "invalid title: has no text in es");
}

#[test]
fn markup_is_a_small_safe_subset() {
    let text =
        "Hills at **dusk** and *dawn*.\nSee [the site](https://example.org/a?b=1&c=2).\n\n- one\n- two <3";
    markup::check(text).unwrap();
    assert_eq!(
        markup::html(text),
        "<p>Hills at <b>dusk</b> and <i>dawn</i>.<br>See <a href=\"https://example.org/a?b=1&amp;c=2\">the site</a>.</p>\n\n<ul><li>one</li><li>two &lt;3</li></ul>"
    );
    assert_eq!(
        markup::plain(text),
        "Hills at dusk and dawn.\nSee the site (https://example.org/a?b=1&c=2).\n\n• one\n• two <3"
    );
    for bad in [
        "<script>x</script>",
        "![photo](https://example.org/p.png)",
        "[site](http://example.org)",
        "[x](javascript:alert(1))",
        "<img src=x>",
        "**open",
        "A 5*5 grid",
        "Uses C++ and *nix tools",
    ] {
        assert!(markup::check(bad).is_err(), "{bad}");
    }
    assert_eq!(markup::html("**open"), "<p>**open</p>");
    assert_eq!(markup::html("A 5*5 grid of *tiles*"), "<p>A 5*5 grid of <i>tiles</i></p>");
    assert_eq!(markup::plain("Uses C++ and *nix tools"), "Uses C++ and *nix tools");
    assert_eq!(markup::plain("a * b and ** c"), "a * b and ** c");
    assert_eq!(markup::plain("A 5\\*5 grid, **very** \\*fine\\*"), "A 5*5 grid, very *fine*");
    markup::check("A 5\\*5 grid and a * b").unwrap();
    assert_eq!(markup::html("[x](javascript:alert(1))"), "<p>[x](javascript:alert(1))</p>");
    assert_eq!(markup::plain("ñandú *ágil*"), "ñandú ágil");
    assert_eq!(markup::verbatim("A *plain* <text>\n\nby me"), "<p>A *plain* &lt;text&gt;</p><p>by me</p>");
}
