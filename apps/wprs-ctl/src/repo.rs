use crate::keys::Policy;
use clap::{Args, Subcommand};
use std::path::PathBuf;

mod act;

pub use act::run;

const REASONS: [&str; 5] = ["malware", "license", "takedown", "broken", "other"];

#[derive(Subcommand, Debug)]
pub enum Repo {
    #[command(about = "Create a key pair NAME.key and NAME.pub and print its fingerprint")]
    Keygen {
        #[arg(help = "Name of the key files")]
        name: String,
        #[arg(long, help = "Leave the secret key unencrypted, for online keys used by automation")]
        no_password: bool,
    },
    #[command(about = "Create a repository in a folder; sign its root next")]
    Init {
        #[arg(help = "Empty folder for the repository")]
        dir: PathBuf,
        #[arg(long, help = "Name of the repository")]
        name: String,
        #[arg(long = "root-key", value_name = "PUB", required = true, help = "Public key file or text")]
        root_keys: Vec<String>,
        #[arg(long = "online-key", value_name = "PUB", required = true, help = "Public key file or text")]
        online_keys: Vec<String>,
        #[command(flatten)]
        policy: Policy,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "Sign the newest root with a root key")]
    SignRoot {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(long, help = "Secret key file")]
        key: PathBuf,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "Print the signing state of the root and the query that anchors the repository")]
    Status {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "Add packages to the catalog; publish them next")]
    Add {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(required = true, value_name = "WPAK", help = "Package file to add")]
        files: Vec<PathBuf>,
        #[arg(long, help = "Author id the packages must be signed by")]
        author: Option<String>,
        #[arg(
            long,
            help = "Leave the new versions out of browsing and search; installing by id still works"
        )]
        unlisted: bool,
        #[arg(long, help = "Print one JSON object per package")]
        json: bool,
    },
    #[command(
        about = "Unlist a package or one version: out of browsing and search, still installable by id; publish next"
    )]
    Unlist {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(help = "Package id")]
        id: String,
        #[arg(long, help = "Version to unlist; every version when left out")]
        version: Option<String>,
    },
    #[command(
        about = "Feature a package of the repository, signed into it, with an optional note; publish next"
    )]
    Feature {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(help = "Package id")]
        id: String,
        #[arg(long, help = "Short note shown with the package")]
        note: Option<String>,
    },
    #[command(about = "Stop featuring a package; publish next")]
    Unfeature {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(help = "Package id")]
        id: String,
    },
    #[command(about = "List an unlisted package or version again; publish next")]
    ListAgain {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(help = "Package id")]
        id: String,
        #[arg(long, help = "Version to list again; every version when left out")]
        version: Option<String>,
    },
    #[command(about = "Revoke a package or one of its versions; publish next")]
    Revoke {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(help = "Package id")]
        id: String,
        #[arg(long, help = "Version to revoke; every version when left out")]
        version: Option<String>,
        #[arg(long, value_parser = REASONS, help = "Why the package is revoked")]
        reason: String,
        #[arg(long, default_value = "", help = "Message shown to whoever installed it")]
        message: String,
    },
    #[command(about = "Withdraw the revocation of a package or of one version; publish next")]
    Unrevoke {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(help = "Package id")]
        id: String,
        #[arg(long, help = "The version revoked on its own; without it, the revocation of every version")]
        version: Option<String>,
    },
    #[command(about = "Revoke an author key; publish next")]
    RevokeKey {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(help = "Fingerprint of the author key")]
        fingerprint: String,
        #[arg(long, value_parser = REASONS, help = "Why the key is revoked")]
        reason: String,
    },
    #[command(about = "Sign and write the index after add or revoke")]
    Publish(Publish),
    #[command(about = "Sign the unchanged index again with a new expiry; run it before the index expires")]
    Refresh(Publish),
    #[command(about = "Write a new root from the current one and the given changes")]
    RotateRoot {
        #[arg(help = "Repository folder")]
        dir: PathBuf,
        #[arg(
            long = "root-key",
            value_name = "PUB",
            help = "Public key file or text; replaces the root keys"
        )]
        root_keys: Vec<String>,
        #[arg(
            long = "online-key",
            value_name = "PUB",
            help = "Public key file or text; replaces the online keys"
        )]
        online_keys: Vec<String>,
        #[arg(long = "remove-author", value_name = "ID", help = "Author to remove; repeat for several")]
        removed: Vec<String>,
        #[command(flatten)]
        policy: Policy,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "Verify a published repository as a client would, installing nothing")]
    Verify {
        #[arg(value_name = "URL|DIR", help = "URL of the repository with its anchor query, or its folder")]
        url: String,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
}

#[derive(Args, Debug)]
pub struct Publish {
    #[arg(help = "Repository folder")]
    dir: PathBuf,
    #[arg(long, help = "Secret file of an online key")]
    key: PathBuf,
    #[arg(long, default_value_t = 7, help = "Days until the signed index expires")]
    days: u32,
    #[arg(long, help = "Print the result as JSON")]
    json: bool,
}
