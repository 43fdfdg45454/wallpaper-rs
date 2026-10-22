use crate::editing::open;
use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::{Path, PathBuf};
use wprs_core::edit::{Depth, Edit, Source};
use wprs_depth::{Places, Stage, beside, read, write};

const MAX_EDGE: u32 = 4096;

#[derive(Subcommand, Debug)]
pub enum Maps {
    #[command(about = "Estimate the depth map with the downloaded model, showing progress; Ctrl-C cancels")]
    Generate {
        path: PathBuf,
        #[arg(long, help = "Estimate it again even when a map exists")]
        force: bool,
    },
    #[command(about = "Use a picture as the depth map, saved next to the image; white is near")]
    Import { path: PathBuf, file: PathBuf },
    #[command(about = "Save the depth map as a 16-bit grey PNG; next to the image without a file")]
    Export { path: PathBuf, file: Option<PathBuf> },
}

pub fn key(path: &Path, edit: &Edit) -> Result<String> {
    match edit.source.sha256.as_str() {
        "" => Ok(Source::read(path, [0; 2])?.sha256),
        sha => Ok(sha.to_owned()),
    }
}

pub fn map(path: &Path, edit: &Edit) -> Result<Option<wprs_depth::Map>> {
    let depth = edit.depth.clone().unwrap_or_default();
    Places::default().map(path, &key(path, edit)?, &depth, MAX_EDGE)
}

pub fn run(command: Maps) -> Result<()> {
    let places = Places::default();
    let (path, edit) = match &command {
        Maps::Generate { path, .. } | Maps::Import { path, .. } | Maps::Export { path, .. } => {
            (path, open(path)?)
        }
    };
    let (sha, depth) = (key(path, &edit)?, edit.depth.clone().unwrap_or_default());
    let found = places.find(path, &sha, &depth);
    let shown = match command {
        Maps::Generate { force: false, .. } if found.is_some() => found,
        Maps::Generate { path, .. } => Some(generate(&places, &path, &sha, &depth)?),
        Maps::Import { path, file } => Some(places.import(&file, &path, &sha)?),
        Maps::Export { path, file } => {
            let from = found.context("the image has no depth map; generate or import one first")?;
            let to = file.unwrap_or_else(|| beside(&path));
            write(&read(&from)?, &to)?;
            Some(to)
        }
    };
    shown.iter().for_each(|p| println!("{}", p.display()));
    Ok(())
}

fn generate(places: &Places, path: &Path, sha: &str, depth: &Depth) -> Result<PathBuf> {
    wprs_depth::background();
    let started = std::time::Instant::now();
    let made = wprs_depth::generate(places, (path, sha), depth, None, &mut |stage| match stage {
        Stage::Loading => eprintln!("loading the depth model"),
        Stage::Estimating => eprintln!("estimating depth"),
        Stage::Saving => eprintln!("saving the depth map"),
    })?;
    eprintln!("done in {:.1} s", started.elapsed().as_secs_f32());
    Ok(made)
}
