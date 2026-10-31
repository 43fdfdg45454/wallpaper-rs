use crate::author::Author;
use crate::steps::{action, edit};
use crate::usage;
use anyhow::{Context, Result, anyhow, bail, ensure};
use serde_json::json;
use std::fs;
use std::path::Path;
use wprs_client::Client;
use wprs_editor::actions::visible;
use wprs_editor::starters::{default_id, starter};
use wprs_editor::{Project, assets, inspect, package};

fn describe(project: &Project, json: bool) -> Result<()> {
    let problems = package::problems(&project.dir, &project.doc);
    let errors = problems.iter().filter(|p| !p.warning).count();
    let m = &project.doc["manifest"];
    let layers = wprs_editor::layers::walk(&project.doc).len();
    let pending = project.pending.is_some();
    if json {
        let files = wprs_editor::check::check_files(&project.dir, &project.doc);
        println!("{}", wprs_editor::outline::outline(project, 0.0, &files));
        return Ok(());
    }
    let text = |k: &str| m[k].as_str().unwrap_or("?").to_owned();
    let title = wprs_editor::meta::title(&project.doc, &wprs_core::locale::languages());
    let (layers, errors) = (count(layers, "layer"), count(errors, "problem"));
    println!("{title} ({} {}): {layers}, {errors}", text("id"), text("version"));
    if pending {
        println!("the app kept unsaved changes; wprs-ctl scene recover restores or drops them");
    }
    Ok(())
}

fn count(n: usize, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread().enable_all().build()?)
}

pub fn run(command: Author) -> Result<()> {
    let command = match action(command) {
        Ok((dir, action)) => return edit(&dir, action),
        Err(command) => *command,
    };
    match command {
        Author::New { dir, from, id, title, locale } => {
            let name = fs::canonicalize(
                dir.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")),
            )?
            .join(dir.file_name().context("name the new folder")?);
            let folder = name.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let id = id.unwrap_or_else(|| default_id(&folder));
            let locale = locale.unwrap_or_else(wprs_editor::starters::locale);
            let doc =
                starter(&from, &id, &title.unwrap_or(folder), &locale).map_err(|e| usage(e.to_string()))?;
            Project::create(&dir, doc)?;
        }
        Author::Open { dir, from, json } => {
            if let Some(file) = from {
                package::extract(&file, &dir)?;
            }
            describe(&Project::open(&dir)?, json)?;
        }
        Author::Show { dir, pointer } => {
            let project = Project::open(&dir)?;
            let shown = project
                .doc
                .pointer(pointer.as_deref().unwrap_or(""))
                .context("nothing is at that pointer")?;
            println!("{}", serde_json::to_string_pretty(shown)?);
        }
        Author::Fields { dir, pointer } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&{
                    let project = Project::open(&dir)?;
                    inspect::fields_own(&project.doc, &pointer, &project.effects())
                })?
            );
        }
        Author::Prune { dir } => {
            assets::prune(&dir, &Project::open(&dir)?.doc)?.iter().for_each(|p| println!("{p}"))
        }
        Author::History { dir } => {
            let project = Project::open(&dir)?;
            let (done, undone) = project.history.labels();
            let step = |label: &&str| wprs_editor::history::step(label).local();
            done.iter().for_each(|label| println!("{}", step(label)));
            undone.iter().for_each(|label| println!("{} (undone)", step(label)));
        }
        Author::Recover { dir, discard } => {
            let mut project = Project::open(&dir)?;
            ensure!(project.pending.is_some(), "there are no unsaved changes to recover");
            project.recover(!discard)?;
            if !discard {
                project.save()?;
            }
        }
        Author::Validate { dir, json } => {
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
        Author::Build { dir, out, key, json } => {
            let mut project = Project::open(&dir)?;
            let out = out.map_or_else(|| package::output(&dir, &project.doc), Ok)?;
            let signer = key.map(|key| crate::keys::signer(&key)).transpose()?;
            let (sha256, size) = package::build(&mut project, &out, signer.as_ref())?;
            match json {
                true => println!("{}", json!({"path": out, "sha256": sha256, "size": size})),
                false => println!("{sha256} {size} {}", out.display()),
            }
        }
        Author::Preview { dir, output, time, stop } => {
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
        Author::Apply { dir, output } => {
            let mut project = Project::open(&dir)?;
            ensure!(!project.dirty() && project.pending.is_none(), "save the scene first");
            let out = package::applied(&project.doc)?;
            package::build(&mut project, &out, None)?;
            let path = out.to_str().context("the path is not UTF-8")?.to_owned();
            runtime()?.block_on(apply(&path, &output))?;
        }
        Author::Schema { property, manifest } => {
            let schema = match (property, manifest) {
                (true, _) => wprs_scene::property_schema(),
                (_, true) => wprs_format::manifest_schema(),
                _ => wprs_scene::schema(),
            };
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        other => bail!("{other:?} is not a scene command"),
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
