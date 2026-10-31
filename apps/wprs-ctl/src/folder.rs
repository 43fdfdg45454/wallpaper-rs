use crate::author::DIR;
use crate::authoring::count;
use anyhow::{Context, Result, anyhow, ensure};
use clap::Subcommand;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use wprs_client::Client;
use wprs_editor::actions::visible;
use wprs_editor::{Project, assets, package};

#[derive(Subcommand, Debug)]
pub enum Folder {
    #[command(about = "Delete the files in assets/ that the scene does not use")]
    Prune {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
    #[command(about = "List the changes that undo and redo walk through")]
    History {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
    #[command(about = "Restore the unsaved changes the app kept for a folder, or drop them with --discard")]
    Recover {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long)]
        discard: bool,
    },
    #[command(about = "Check the scene and its files; exits with 1 when something is wrong")]
    Validate {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, help = "Print one JSON object per problem")]
        json: bool,
    },
    #[command(about = "Build the .wpak package, signed with --key, as wprs-ctl pack build does")]
    Build {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(short, long, help = "Package file; <id>-<version>.wpak next to the scene folder when omitted")]
        out: Option<PathBuf>,
        #[arg(long, help = "Secret key file of the author")]
        key: Option<PathBuf>,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "Show the scene of the folder on the desktop for 60 seconds without applying it")]
    Preview {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(short, long, default_value = "", help = crate::cli::OUTPUT)]
        output: String,
        #[arg(long, default_value_t = -1.0, allow_hyphen_values = true, help = "Hold this second; plays when negative")]
        time: f64,
        #[arg(long, help = "End the preview")]
        stop: bool,
    },
    #[command(about = "Build the scene into the library and show it on an output")]
    Apply {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(short, long, default_value = "", help = crate::cli::OUTPUT)]
        output: String,
    },
    #[command(
        about = "Print the JSON Schema of scenes, of user properties with --property or of manifests with --manifest"
    )]
    Schema {
        #[arg(long)]
        property: bool,
        #[arg(long, conflicts_with = "property")]
        manifest: bool,
    },
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread().enable_all().build()?)
}

pub(crate) fn run(command: Folder) -> Result<()> {
    match command {
        Folder::Prune { dir } => {
            assets::prune(&dir, &Project::open(&dir)?.doc)?.iter().for_each(|p| println!("{p}"))
        }
        Folder::History { dir } => {
            let project = Project::open(&dir)?;
            let (done, undone) = project.history.labels();
            let step = |label: &&str| wprs_editor::history::step(label).local();
            done.iter().for_each(|label| println!("{}", step(label)));
            undone.iter().for_each(|label| println!("{} (undone)", step(label)));
        }
        Folder::Recover { dir, discard } => {
            let mut project = Project::open(&dir)?;
            ensure!(project.pending.is_some(), "there are no unsaved changes to recover");
            project.recover(!discard)?;
            if !discard {
                project.save()?;
            }
        }
        Folder::Validate { dir, json } => {
            let project = Project::open(&dir)?;
            let problems = package::problems(&project.dir, &project.doc);
            for p in &problems {
                match json {
                    true => println!("{}", serde_json::to_string(p)?),
                    false => {
                        println!(
                            "{}{}: {}",
                            if p.warning { "warning: " } else { "" },
                            p.pointer,
                            p.said.local()
                        )
                    }
                }
            }
            let errors = problems.iter().filter(|p| !p.warning).count();
            ensure!(errors == 0, "{}", count(errors, "problem"));
        }
        Folder::Build { dir, out, key, json } => {
            let mut project = Project::open(&dir)?;
            let out = out.map_or_else(|| package::output(&dir, &project.doc), Ok)?;
            let signer = key.map(|key| crate::keys::signer(&key)).transpose()?;
            let (sha256, size) = package::build(&mut project, &out, signer.as_ref())?;
            match json {
                true => println!("{}", json!({"path": out, "sha256": sha256, "size": size})),
                false => println!("{sha256} {size} {}", out.display()),
            }
        }
        Folder::Preview { dir, output, time, stop } => {
            let project = Project::open(&dir)?;
            let document =
                json!({"manifest": project.doc["manifest"], "scene": visible(&project.doc)}).to_string();
            let (dir, document) = (fs::canonicalize(&dir)?, if stop { String::new() } else { document });
            let path = dir.to_str().context("the path is not UTF-8")?.to_owned();
            runtime()?.block_on(async {
                Ok::<_, anyhow::Error>(
                    Client::connect().await?.daemon.preview_scene(&output, &path, &document, time).await?,
                )
            })?;
        }
        Folder::Apply { dir, output } => {
            let mut project = Project::open(&dir)?;
            ensure!(!project.dirty() && project.pending.is_none(), "save the scene first");
            let out = package::applied(&project.doc)?;
            package::build(&mut project, &out, None)?;
            let path = out.to_str().context("the path is not UTF-8")?.to_owned();
            runtime()?.block_on(apply(&path, &output))?;
        }
        Folder::Schema { property, manifest } => {
            let schema = match (property, manifest) {
                (true, _) => wprs_scene::property_schema(),
                (_, true) => wprs_format::manifest_schema(),
                _ => wprs_scene::schema(),
            };
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
    }
    Ok(())
}

async fn apply(path: &str, output: &str) -> Result<()> {
    let client = Client::connect().await?;
    let (_, job) =
        client.run_job(async { Ok(((), client.library.import(&[path], false).await?)) }, |_, _| {}).await?;
    let report = job.output.unwrap_or_default();
    let uid = report["imported"][0]["uid"].as_str().map(String::from);
    let failed =
        report["failed"][0]["error"].as_str().unwrap_or("the library refused the package").to_owned();
    let uid = uid.ok_or_else(|| anyhow!(failed))?;
    client.library.apply(&uid, output, "", "").await?;
    println!("{uid}");
    Ok(())
}
