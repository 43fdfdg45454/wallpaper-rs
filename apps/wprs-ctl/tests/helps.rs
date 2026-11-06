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

fn texts(command: &clap::Command, out: &mut Vec<String>) {
    out.extend(command.get_about().map(|a| a.to_string()));
    out.extend(command.get_arguments().filter_map(|a| a.get_help()).map(|h| h.to_string()));
    command.get_subcommands().for_each(|s| texts(s, out));
}

#[test]
fn every_help_text_has_a_translation() {
    let catalog = wprs_core::locale::parse(include_str!("../../../po/es/wprs-ctl.po"));
    let mut all = Vec::new();
    texts(&Cli::command(), &mut all);
    let missing: Vec<_> = all.iter().filter(|t| !catalog.contains_key(*t)).collect();
    assert!(missing.is_empty(), "{} help texts the catalog lacks:\n{missing:#?}", missing.len());
}

#[test]
fn the_help_speaks_the_language_of_the_desktop() {
    let help = |lang: &str| {
        let mut run = std::process::Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
        run.args(["repo", "rotate-root", "--help"]).env("LANG", lang);
        let out = run.env_remove("LANGUAGE").env_remove("LC_ALL").env_remove("LC_MESSAGES").output().unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    assert!(help("C.UTF-8").contains("Drop every mirror of the root"));
    let spanish = help("es_ES.UTF-8");
    assert!(
        spanish.contains("Quitar todas las réplicas de la raíz") && spanish.contains("Imprimir la ayuda"),
        "{spanish}"
    );
}
