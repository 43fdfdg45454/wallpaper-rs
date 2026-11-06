use crate::Cli;
use clap::{Arg, ArgAction, ArgMatches, Command, CommandFactory, FromArgMatches};
use std::collections::HashMap;
use std::sync::OnceLock;

const CATALOGS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
static FOUND: OnceLock<HashMap<String, String>> = OnceLock::new();

pub(crate) const fn told(text: &'static str) -> &'static str {
    text
}

pub fn tr(text: &str) -> String {
    FOUND.get().and_then(|f| f.get(text)).cloned().unwrap_or_else(|| text.to_owned())
}

pub fn fill(text: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = tr(text);
    for (i, a) in args.iter().enumerate().rev() {
        out = out.replace(&format!("%{}", i + 1), &a.to_string());
    }
    out
}

pub fn parse() -> Cli {
    let found = FOUND.get_or_init(|| wprs_core::locale::catalog(CATALOGS, &wprs_core::locale::languages()));
    let command = Cli::command();
    if found.is_empty() {
        return Cli::from_arg_matches(&command.get_matches()).unwrap_or_else(|e| e.exit());
    }
    let command = translated(command, found, true);
    let matches = command.clone().try_get_matches().unwrap_or_else(|e| crate::refusing::exit(e));
    if let Some(path) = asked(&matches) {
        let argv = std::iter::once("wprs-ctl".to_owned()).chain(path).chain(["--help".to_owned()]);
        match command.try_get_matches_from(argv) {
            Ok(_) => std::process::exit(0),
            Err(e) => crate::refusing::exit(e),
        }
    }
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| crate::refusing::exit(e))
}

fn asked(matches: &ArgMatches) -> Option<Vec<String>> {
    let (mut names, mut at) = (Vec::new(), matches);
    while let Some((name, sub)) = at.subcommand() {
        if name == "help" {
            names.extend(sub.get_many::<String>("command").into_iter().flatten().cloned());
            return Some(names);
        }
        names.push(name.to_owned());
        at = sub;
    }
    None
}

fn shown(arg: &Arg, tr: &dyn Fn(String) -> String) -> String {
    let mut extra = String::new();
    let takes = arg.get_action().takes_values();
    let defaults: Vec<_> =
        arg.get_default_values().iter().map(|v| v.to_string_lossy().into_owned()).collect();
    if takes && !arg.is_hide_default_value_set() && !defaults.is_empty() {
        extra += &format!(" [{}: {}]", tr(told("default").into()), defaults.join(", "));
    }
    let possible: Vec<_> = arg
        .get_possible_values()
        .into_iter()
        .filter(|p| !p.is_hide_set())
        .map(|p| p.get_name().to_owned())
        .collect();
    if takes && !arg.is_hide_possible_values_set() && !possible.is_empty() {
        extra += &format!(" [{}: {}]", tr(told("possible values").into()), possible.join(", "));
    }
    extra
}

pub fn translated(command: Command, found: &HashMap<String, String>, root: bool) -> Command {
    let tr = |text: String| found.get(&text).cloned().unwrap_or(text);
    let mut command = match command.get_about().map(|a| tr(a.to_string())) {
        Some(about) => command.about(about),
        None => command,
    };
    if let Some(after) = command.get_after_long_help().map(|a| tr(a.to_string())) {
        command = command.after_long_help(after);
    }
    let fixed = |text: &'static str| -> &'static str { Box::leak(tr(text.into()).into_boxed_str()) };
    let (options, places) = (fixed(told("Options")), fixed(told("Arguments")));
    let ids: Vec<_> = command.get_arguments().map(|a| a.get_id().clone()).collect();
    for id in ids {
        command = command.mut_arg(id, |arg| {
            let heading = if arg.is_positional() { places } else { options };
            let extra = shown(&arg, &tr);
            let help = arg.get_help().map(|h| tr(h.to_string())).unwrap_or_default();
            let takes = arg.get_action().takes_values();
            let arg = arg.help_heading(heading).hide_default_value(takes).hide_possible_values(takes);
            match (help + &extra).trim() {
                "" => arg,
                help => arg.help(help.to_owned()),
            }
        });
    }
    let usage = tr(told("Usage").into());
    command = command
        .subcommand_help_heading(fixed(told("Commands")))
        .help_template(format!("{{about-with-newline}}\n{usage}: {{usage}}\n\n{{all-args}}{{after-help}}"));
    let help = Arg::new("help").short('h').long("help").action(ArgAction::Help);
    command =
        command.disable_help_flag(true).arg(help.help(tr(told("Print help").into())).help_heading(options));
    if root {
        let version = Arg::new("version").short('V').long("version").action(ArgAction::Version);
        let version = version.help(tr(told("Print version").into())).help_heading(options);
        command = command.disable_version_flag(true).arg(version);
    }
    let names: Vec<String> = command.get_subcommands().map(|s| s.get_name().to_owned()).collect();
    if !names.is_empty() {
        let about = tr(told("Print this message or the help of the given subcommand(s)").into());
        let path = Arg::new("command").num_args(0..).action(ArgAction::Append).value_name("COMMAND");
        let path = path.help(tr(told("The command whose help to print").into())).help_heading(places);
        let asking = Command::new("help").about(about).arg(path).disable_help_flag(true);
        command = command.disable_help_subcommand(true).subcommand(asking);
    }
    names.into_iter().fold(command, |c, name| c.mut_subcommand(name, |s| translated(s, found, false)))
}
