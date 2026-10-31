use crate::author::Author;
use crate::steps::{action, edit};
use crate::usage;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use wprs_editor::starters::{default_id, starter};
use wprs_editor::{Project, inspect, package};

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

pub(crate) fn count(n: usize, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
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
            let doc = starter(&from, &id, &title.unwrap_or(folder), &locale)
                .map_err(|e| usage(wprs_editor::told(&e)))?;
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
        Author::Folder(command) => crate::folder::run(command)?,
        other => bail!("{other:?} is not a scene command"),
    }
    Ok(())
}
