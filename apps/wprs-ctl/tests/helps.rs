use clap::{ArgAction, CommandFactory};
use wprs_ctl::Cli;

fn bare(command: &clap::Command, path: &str, out: &mut Vec<String>) {
    for arg in command.get_arguments() {
        let id = arg.get_id().as_str();
        let told = arg.get_help().is_some() || arg.get_long_help().is_some();
        let own = matches!(arg.get_action(), ArgAction::Help | ArgAction::Version);
        if !told && !own {
            out.push(format!("{path} {id}"));
        }
    }
    for sub in command.get_subcommands() {
        bare(sub, &format!("{path} {}", sub.get_name()), out);
    }
}

#[test]
fn every_argument_of_every_command_says_what_it_takes() {
    let cli = Cli::command();
    let mut missing = Vec::new();
    for sub in cli.get_subcommands() {
        bare(sub, sub.get_name(), &mut missing);
    }
    assert!(missing.is_empty(), "{} arguments without help:\n{}", missing.len(), missing.join("\n"));
}
