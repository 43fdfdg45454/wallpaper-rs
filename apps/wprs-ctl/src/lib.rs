mod about;
mod account;
mod args;
pub mod author;
mod authoring;
mod bench;
mod calls;
mod caution;
pub mod cli;
mod clipping;
mod consent;
mod daemon;
mod depth;
pub mod edit;
mod editing;
mod facts;
mod fielding;
mod filtering;
mod folder;
mod glowing;
mod gpus;
pub mod input;
pub mod inputs;
pub mod items;
pub mod keys;
mod lamped;
mod lamping;
mod layering;
pub mod library;
mod limbing;
mod marking;
pub mod models;
pub mod moderate;
pub mod pack;
pub mod pkg;
pub mod playlists;
pub mod private;
pub mod profiles;
pub mod props;
pub mod propsets;
pub mod providers;
pub mod remote;
pub mod repo;
mod rigging;
pub mod server;
pub mod serving;
pub mod show;
mod steps;
mod swarmed;
mod swarming;
mod swinging;
mod tended;
mod tending;
mod undoing;
mod userprops;

pub use calls::CALLS;
pub use cli::{Cli, Command};

use clap::Parser;
use std::fmt;
use std::process::ExitCode;
use wprs_client::{error_name, explain};

#[derive(Debug)]
pub struct Usage(pub String);

impl fmt::Display for Usage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Usage {}

pub fn usage(message: impl Into<String>) -> anyhow::Error {
    Usage(message.into()).into()
}

pub fn main() -> ExitCode {
    wprs_scene::serve_check();
    wprs_scene::check_out_of_process(true);
    wprs_core::stopped_effects().inspect(|p| wprs_scene::keep_strikes(p));
    let result = match Cli::parse().command {
        Command::Pack(command) => pack::run(command),
        Command::Scene(command) => bench::run(command),
        Command::Repo(command) => repo::run(command),
        Command::Edit(command) => edit::run(command),
        Command::Models(command) => models::run(command),
        Command::Moderate(command) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(anyhow::Error::from)
            .and_then(|runtime| runtime.block_on(moderate::run(command))),
        command => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(anyhow::Error::from)
            .and_then(|runtime| runtime.block_on(daemon::run(command))),
    };
    wprs_scene::end_trials();
    let Err(error) = result else {
        wprs_render::finish_builds(0);
        return ExitCode::SUCCESS;
    };
    let error = error.downcast::<zbus::Error>().map_or_else(|e| e, explain);
    eprintln!("Error: {}", show::clean(&wprs_editor::told(&error)));
    let code = if misused(&error) { 2 } else { 1 };
    wprs_render::finish_builds(i32::from(code));
    ExitCode::from(code)
}

fn misused(error: &anyhow::Error) -> bool {
    error.is::<Usage>() || error_name(error) == Some("org.freedesktop.DBus.Error.InvalidArgs")
}
