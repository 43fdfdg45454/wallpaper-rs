use crate::Cli;
use clap::{Arg, ArgAction, Command, CommandFactory, FromArgMatches};
use std::collections::HashMap;

const CATALOGS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

fn told(text: &'static str) -> &'static str {
    text
}

pub fn parse() -> Cli {
    let found = wprs_core::locale::catalog(CATALOGS, &wprs_core::locale::languages());
    let command = Cli::command();
    let command = if found.is_empty() { command } else { translated(command, &found, true) };
    Cli::from_arg_matches(&command.get_matches()).unwrap_or_else(|e| e.exit())
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
            let arg = arg.help_heading(heading);
            match arg.get_help().map(|h| tr(h.to_string())) {
                Some(help) => arg.help(help),
                None => arg,
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
    names.into_iter().fold(command, |c, name| c.mut_subcommand(name, |s| translated(s, found, false)))
}
