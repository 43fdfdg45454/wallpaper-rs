use crate::told::{fill, told, tr};
use clap::error::ErrorKind;

const SAID: &[&str] = &[
    told("unexpected argument '%1' found"),
    told("the argument '%1' cannot be used multiple times"),
    told("the argument '%1' cannot be used with '%2'"),
    told("the argument '%1' cannot be used with one or more of the other specified arguments"),
    told("the argument '%1' cannot be used with:"),
    told("the subcommand '%1' cannot be used with '%2'"),
    told("the subcommand '%1' cannot be used with one or more of the other specified arguments"),
    told("the subcommand '%1' cannot be used with:"),
    told("equal sign is needed when assigning values to '%1'"),
    told("a value is required for '%1' but none was supplied"),
    told("unexpected value '%1' for '%2' found; no more were expected"),
    told("invalid value '%1' for '%2'"),
    told("[possible values: %1]"),
    told("[subcommands: %1]"),
    told("unrecognized subcommand '%1'"),
    told("the following required arguments were not provided:"),
    told("'%1' requires a subcommand but one was not provided"),
    told("%1 values required by '%2'; only %3 were provided"),
    told("%1 values required by '%2'; only %3 was provided"),
    told("%1 values required for '%2' but %3 were provided"),
    told("%1 values required for '%2' but %3 was provided"),
    told("For more information, try '%1'."),
    told("a similar argument exists: '%1'"),
    told("a similar subcommand exists: '%1'"),
    told("a similar value exists: '%1'"),
    told("some similar arguments exist: %1"),
    told("some similar subcommands exist: %1"),
    told("some similar values exist: %1"),
    told("to pass '%1' as a value, use '%2'"),
    told("subcommand '%1' exists; to use it, remove the '%2' before it"),
    told("'%1' exists"),
    told("one of the values isn't valid for an argument"),
    told("unexpected argument found"),
    told("unrecognized subcommand"),
    told("equal is needed when assigning values to one of the arguments"),
    told("invalid value for one of the arguments"),
    told("unexpected value for an argument found"),
    told("more values required for an argument"),
    told("too many or too few values for an argument"),
    told("an argument cannot be used with one or more of the other specified arguments"),
    told("one or more required arguments were not provided"),
    told("a subcommand is required but one was not provided"),
    told("invalid UTF-8 was detected in one or more arguments"),
];
const LEADS: &[&str] = &[told("error"), told("tip"), told("Usage")];

fn matched<'a>(template: &str, line: &'a str) -> Option<(Vec<&'a str>, usize)> {
    let mut parts = template.split('%');
    let first = parts.next()?;
    let mut at = line.strip_prefix(first).map(|_| first.len())?;
    let mut caught = Vec::new();
    for part in parts {
        let literal = part.trim_start_matches(|c: char| c.is_ascii_digit());
        if literal.is_empty() {
            caught.push(&line[at..]);
            at = line.len();
            continue;
        }
        let found = line[at..].find(literal)?;
        caught.push(&line[at..at + found]);
        at += found + literal.len();
    }
    Some((caught, at))
}

pub fn said(line: &str) -> String {
    let body = line.trim_start();
    let mut out = line[..line.len() - body.len()].to_owned();
    let mut rest = body;
    if let Some(lead) = LEADS.iter().find(|l| rest.starts_with(&format!("{l}: "))) {
        out += &format!("{}: ", tr(lead));
        rest = &rest[lead.len() + 2..];
    }
    let mut order: Vec<&str> = SAID.to_vec();
    order.sort_by_key(|t| std::cmp::Reverse(t.len()));
    for template in order {
        if let Some((caught, end)) = matched(template, rest) {
            let args: Vec<&dyn std::fmt::Display> =
                caught.iter().map(|c| c as &dyn std::fmt::Display).collect();
            return out + &fill(template, &args) + &rest[end..];
        }
    }
    out + rest
}

pub fn exit(e: clap::Error) -> ! {
    let shown = [
        ErrorKind::DisplayHelp,
        ErrorKind::DisplayVersion,
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
    ];
    if shown.contains(&e.kind()) {
        e.exit()
    }
    let text: Vec<String> = e.render().to_string().lines().map(said).collect();
    let text = text.join("\n") + "\n";
    match e.use_stderr() {
        true => eprint!("{text}"),
        false => print!("{text}"),
    }
    std::process::exit(e.exit_code())
}
