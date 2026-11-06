use clap::Parser;
use wprs_ctl::cli::{Cli, Command};
use wprs_ctl::providers::Provider;
use wprs_ctl::wallhaven::Wallhaven;

fn pairs(line: &str) -> Result<Vec<(String, String)>, String> {
    let words: Vec<&str> = line.split(' ').filter(|w| *w != "-").collect();
    run(&words)
}

fn run(words: &[&str]) -> Result<Vec<(String, String)>, String> {
    let args = ["wprs-ctl", "provider", "wallhaven", "set", "art"].into_iter().chain(words.iter().copied());
    let parsed = Cli::try_parse_from(args).map_err(|e| e.to_string())?.command;
    let Command::Provider(Provider::Wallhaven(command)) = parsed else { unreachable!("a wallhaven set") };
    let Wallhaven::Set { filters, .. } = *command else { unreachable!("a wallhaven set") };
    let pairs = filters.pairs().map_err(|e| e.to_string())?;
    Ok(pairs.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())
}

fn has(pairs: &[(String, String)], key: &str, value: &str) -> bool {
    pairs.iter().any(|(k, v)| k == key && v == value)
}

#[test]
fn flags_become_the_options_of_the_daemon() {
    let all = pairs("--categories general,people --purity sfw --sorting toplist --top-range 3M --ratios 16x9 --colors 660000,ffffff --collection alice/7 --max-items 48").unwrap();
    for (key, value) in [
        ("categories", "101"),
        ("purity", "100"),
        ("sorting", "toplist"),
        ("top_range", "3M"),
        ("ratios", "16x9"),
        ("colors", "660000,ffffff"),
        ("collection", "alice/7"),
        ("max_items", "48"),
    ] {
        assert!(has(&all, key, value), "{key}");
    }
    assert!(has(&pairs("--categories 011").unwrap(), "categories", "011"));
    assert!(run(&[]).unwrap().is_empty());
}

#[test]
fn wrong_values_are_refused_before_anything_is_sent() {
    for (line, hint) in [
        ("--sorting newest", "sorting must be one of"),
        ("--order up", "order must be one of"),
        ("--top-range 2d", "top-range must be one of"),
        ("--categories cats", "categories takes"),
        ("--purity nsfw,wild", "purity takes"),
        ("--ratios wide", "ratios"),
        ("--colors 123456", "colors"),
        ("--resolutions big", "resolutions"),
        ("--collection 12", "collection"),
        ("--tag anime", "tag"),
    ] {
        let refused = pairs(line).unwrap_err();
        assert!(refused.contains(hint), "{line}: {refused}");
    }
}
