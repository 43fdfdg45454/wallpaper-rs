use crate::consent::{consent, list, progress};
use crate::usage;
use anyhow::{Context, Result, bail};
use clap::Subcommand;
use std::path::PathBuf;
use wprs_depth::models::{assets, find};
use wprs_depth::{Asset, Places};

const ID: &str = "depth-anything-v2-small for the depth model, onnxruntime for the library that runs it";

#[derive(Subcommand, Debug)]
pub enum Models {
    #[command(
        about = "List the files depth maps need, with size, license, host and whether they are installed"
    )]
    List {
        #[arg(long, help = crate::cli::JSON)]
        json: bool,
    },
    #[command(
        about = "Download a file after showing its size, license and host; Ctrl-C stops and keeps what arrived"
    )]
    Download {
        #[arg(help = ID)]
        id: String,
        #[arg(long, help = "Accept the download without asking")]
        yes: bool,
    },
    #[command(about = "Check the SHA-256 of installed files")]
    Verify {
        #[arg(help = ID)]
        id: Option<String>,
    },
    #[command(about = "Delete a downloaded file")]
    Remove {
        #[arg(help = ID)]
        id: String,
    },
    #[command(about = "Install a file fetched by other means; its SHA-256 must match")]
    Install {
        #[arg(help = ID)]
        id: String,
        #[arg(long, help = "The model file, or the ONNX Runtime release archive")]
        file: PathBuf,
    },
}

fn asset(id: &str) -> Result<Asset> {
    let known = assets().iter().map(|a| a.id).collect::<Vec<_>>().join(", ");
    find(id).ok_or_else(|| usage(format!("{id} is not one of {known}")))
}

pub fn run(command: Models) -> Result<()> {
    let places = Places::default();
    match command {
        Models::List { json } => list(&places, json)?,
        Models::Download { id, yes } => {
            let asset = asset(&id)?;
            let path = match places.verify(&asset)? {
                true => places.file(&asset),
                false => {
                    consent(&asset, yes)?;
                    let path = places.download(&asset, &mut progress())?;
                    eprintln!();
                    path
                }
            };
            println!("{}", path.display());
        }
        Models::Verify { id } => {
            let chosen = match id {
                Some(id) => vec![asset(&id)?],
                None => assets().into_iter().filter(|a| places.installed(a).is_some()).collect(),
            };
            for a in chosen {
                let fine = places.verify(&a).with_context(|| format!("cannot read {}", a.id))?;
                println!("{}: {}", a.id, if fine { "ok" } else { "missing or damaged" });
                if !fine {
                    bail!("{} needs to be downloaded or installed again", a.id);
                }
            }
        }
        Models::Remove { id } => drop(places.remove(&asset(&id)?)?),
        Models::Install { id, file } => println!("{}", places.install(&asset(&id)?, &file)?.display()),
    }
    Ok(())
}
