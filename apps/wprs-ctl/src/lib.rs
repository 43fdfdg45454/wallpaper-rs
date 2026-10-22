mod account;
mod bench;
mod calls;
pub mod cli;
mod consent;
mod daemon;
mod depth;
pub mod edit;
mod editing;
pub mod input;
pub mod inputs;
pub mod items;
pub mod keys;
pub mod library;
pub mod models;
pub mod pack;
pub mod pkg;
pub mod playlists;
pub mod private;
pub mod profiles;
pub mod props;
pub mod providers;
pub mod remote;
pub mod repo;
pub mod server;
pub mod show;

pub use calls::CALLS;
pub use cli::{Cli, Command};

use clap::Parser;
use std::fmt;
use std::process::ExitCode;
use wprs_client::explain;

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
    let result = match Cli::parse().command {
        Command::Pack(command) => pack::run(command),
        Command::Scene(command) => bench::run(command),
        Command::Repo(command) => repo::run(command),
        Command::Edit(command) => edit::run(command),
        Command::Models(command) => models::run(command),
        command => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(anyhow::Error::from)
            .and_then(|runtime| runtime.block_on(daemon::run(command))),
    };
    let Err(error) = result else { return ExitCode::SUCCESS };
    let error = error.downcast::<zbus::Error>().map_or_else(|e| e, explain);
    eprintln!("Error: {}", show::clean(&format!("{error:#}")));
    ExitCode::from(if error.is::<Usage>() { 2 } else { 1 })
}
