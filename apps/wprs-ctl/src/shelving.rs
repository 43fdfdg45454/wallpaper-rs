use crate::usage;
use anyhow::Result;
use clap::{Args, Subcommand};
use std::collections::BTreeMap;
use std::path::PathBuf;
use wprs_editor::offer::{Offer, make};
use wprs_editor::shelf::Pick;
use wprs_editor::{Project, stock};

const PART: &str = "A part as layer:ID, particles:ID, script:PATH or effect:ID";

#[derive(Args, Debug)]
pub struct Making {
    #[arg(help = "Scene folder the parts come from")]
    scene: PathBuf,
    #[arg(help = "New, empty folder for the assets package")]
    out: PathBuf,
    #[arg(long = "part", required = true, value_name = "KIND:ID", help = PART)]
    parts: Vec<String>,
    #[arg(long, help = "Package id, such as org.example.parts")]
    id: String,
    #[arg(long)]
    title: String,
    #[arg(long, help = "SPDX license of the parts, such as CC-BY-4.0")]
    license: String,
    #[arg(long, help = "Who made the parts, as other authors will credit them")]
    attribution: String,
    #[arg(long, help = "Picture shown for the package")]
    thumbnail: Option<PathBuf>,
    #[arg(long = "part-thumbnail", value_name = "KIND:ID=FILE", help = "Picture shown for one part")]
    part_thumbnails: Vec<String>,
    #[arg(long = "part-title", value_name = "KIND:ID=TITLE", help = "Title of one part")]
    part_titles: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Shelving {
    #[command(
        about = "Make a folder for an assets package from parts of a scene folder; build it with pack build"
    )]
    NewAssets(Box<Making>),
    #[command(about = "List the parts of an assets package with the license and credit of each file")]
    Parts {
        file: PathBuf,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
}

fn pairs(list: &[String]) -> Result<BTreeMap<String, String>> {
    let pair = |t: &String| match t.split_once('=') {
        Some((key, value)) => Ok((Pick::parse(key)?.key(), value.to_owned())),
        None => Err(usage(format!("expected KIND:ID=VALUE, not {t}"))),
    };
    list.iter().map(pair).collect()
}

pub fn run(command: Shelving) -> Result<()> {
    match command {
        Shelving::NewAssets(making) => {
            let Making {
                scene,
                out,
                parts,
                id,
                title,
                license,
                attribution,
                thumbnail,
                part_thumbnails,
                part_titles,
            } = *making;
            let picks = parts.iter().map(|p| Pick::parse(p)).collect::<Result<Vec<_>>>()?;
            let thumbnails =
                pairs(&part_thumbnails)?.into_iter().map(|(k, v)| (k, PathBuf::from(v))).collect();
            let offer = Offer {
                id,
                title,
                license,
                attribution,
                thumbnail,
                thumbnails,
                titles: pairs(&part_titles)?,
            };
            let source = Project::open(&scene)?;
            let made = make(&source.dir, &source.doc, &picks, &out, &offer)?;
            println!("{}", made.dir.display());
            Ok(())
        }
        Shelving::Parts { file, json } => {
            let listed = stock::browse(&file, None)?;
            if json {
                println!("{listed}");
                return Ok(());
            }
            println!(
                "{} {} ({}, {})",
                listed["id"].as_str().unwrap_or("?"),
                listed["version"].as_str().unwrap_or("?"),
                listed["title"].as_str().unwrap_or("?"),
                listed["license"].as_str().unwrap_or("?")
            );
            for part in listed["parts"].as_array().into_iter().flatten() {
                println!(
                    "{} \"{}\"",
                    part["key"].as_str().unwrap_or("?"),
                    part["title"].as_str().unwrap_or("?")
                );
                for file in part["files"].as_array().into_iter().flatten() {
                    let text = |k: &str| file[k].as_str().unwrap_or("-").to_owned();
                    println!("  {} {} {}", text("path"), text("license"), text("attribution"));
                }
            }
            Ok(())
        }
    }
}
