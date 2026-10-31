use crate::edit::Edits;
use crate::usage;
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
use wprs_client::Client;
use wprs_core::edit::{self, Action, Edit, History, Source};
use wprs_render::Renderer;

fn text(e: String) -> anyhow::Error {
    anyhow::Error::msg(e)
}

fn told(e: wprs_core::message::Message) -> anyhow::Error {
    anyhow::Error::msg(e.local())
}

fn refused(e: edit::Error) -> anyhow::Error {
    match e {
        edit::Error::Usage(message) => usage(message.local()),
        edit::Error::Invalid(message) => told(message),
    }
}

pub fn dimensions(size: &str) -> Result<(u32, u32)> {
    let parsed =
        size.split_once('x').and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)));
    parsed.ok_or_else(|| usage(format!("{size} is not WIDTHxHEIGHT")))
}

fn source(path: &Path) -> Result<Source> {
    let (w, h) =
        wprs_render::image_size(path).with_context(|| format!("{} is not a picture", path.display()))?;
    Ok(Source::read(path, [w, h])?)
}

pub fn open(path: &Path) -> Result<Edit> {
    ensure!(path.is_file(), "cannot find {}", path.display());
    let Some((_, edit)) = edit::load(path, &edit::dir()).map_err(told)? else {
        return Ok(Edit { source: source(path)?, ..Edit::default() });
    };
    let bytes = std::fs::metadata(path)?.len();
    let pixels = wprs_render::image_size(path).map(|(w, h)| [w, h]).unwrap_or_default();
    if edit.source.changed(bytes, pixels) {
        eprintln!("warning: {} changed after it was edited", path.display());
    }
    Ok(edit)
}

pub fn keep(path: &Path, before: &Edit, after: &Edit) -> Result<()> {
    edit::commit(after, path, &edit::dir()).map_err(told)?;
    let mut history = History::load(path);
    history.record(before, after);
    history.save(path).map_err(text)
}

pub fn change(path: &Path, action: Action) -> Result<()> {
    let mut edit = open(path)?;
    let before = edit.clone();
    edit.act(action).map_err(refused)?;
    keep(path, &before, &edit)
}

pub fn step(path: &Path, back: bool) -> Result<()> {
    let (current, mut history) = (open(path)?, History::load(path));
    let found = if back { history.undo(&current) } else { history.redo(&current) };
    let edit =
        found.with_context(|| format!("there is nothing to {}", if back { "undo" } else { "redo" }))?;
    edit::commit(&edit, path, &edit::dir()).map_err(told)?;
    history.save(path).map_err(text)
}

pub fn apply(path: &Path, file: &str) -> Result<()> {
    let json = match file {
        "-" => std::io::read_to_string(std::io::stdin())?,
        file => std::fs::read_to_string(file).with_context(|| format!("cannot read {file}"))?,
    };
    ensure!(path.is_file(), "cannot find {}", path.display());
    let mut edit = Edit::parse(&json).map_err(told)?;
    if edit.source == Source::default() {
        edit.source = source(path)?;
    }
    keep(path, &open(path)?, &edit)
}

pub fn preview(path: &Path, changes: Vec<String>, stop: bool) -> Result<()> {
    let path = std::fs::canonicalize(path).with_context(|| format!("cannot find {}", path.display()))?;
    let mut edit = if stop { None } else { Some(open(&path)?) };
    edit.as_mut().map_or(Ok(()), |edit| edit.act(Action::Set { changes }).map_err(refused))?;
    let (path, json) =
        (path.to_str().context("the path is not UTF-8")?, edit.map(|e| e.to_json()).unwrap_or_default());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async { Ok(Client::connect().await?.daemon.preview_edit(path, &json).await?) })
}

pub fn export(path: &Path, out: &Path, before: bool, quality: u8, size: Option<&str>) -> Result<()> {
    let edit = open(path)?;
    let edit = if before { edit.before() } else { edit };
    let renderer = Renderer::new()?;
    let image = wprs_render::decode_image(path, renderer.max_edge())?;
    let depth = if edit.needs_depth() { crate::depth::map(path, &edit)? } else { None };
    let size = size.map(dimensions).transpose()?;
    let pixels = renderer.export_edit(&image, &edit, depth.as_ref(), size)?;
    wprs_render::save(&pixels, out, quality).map_err(|e| match e.kind() {
        std::io::ErrorKind::InvalidInput => usage(e.to_string()),
        _ => anyhow::Error::from(e).context(format!("cannot write {}", out.display())),
    })
}

fn action(command: Edits) -> Option<(PathBuf, Action)> {
    Some(match command {
        Edits::Set { path, changes } => (path, Action::Set { changes }),
        Edits::Crop { path, rect, aspect, rotate, straighten, flip, output } => {
            (path, Action::Crop { rect, aspect, rotate, straighten, flip, output })
        }
        Edits::Curve { path, points, channel, id } => (path, Action::Curve { points, channel, id }),
        Edits::Blur { path, radius, mask, id } => (path, Action::Blur { radius: Some(radius), mask, id }),
        Edits::Vignette { path, amount, midpoint, roundness, feather, id } => {
            (path, Action::Vignette { amount: Some(amount), midpoint, roundness, feather, id })
        }
        Edits::Overlay { path, color, blend, opacity, mask, id } => {
            (path, Action::Overlay { color: Some(color), blend, opacity, mask, id })
        }
        Edits::Mask { path, name, spec, invert } => {
            (path, Action::Mask { name, spec, invert: invert.then_some(true) })
        }
        Edits::Order { path, ids } => (path, Action::Order { ids }),
        Edits::Span { path, zoom, center } => (path, Action::Span { zoom, center }),
        Edits::Reset { path, ids } => (path, Action::Reset { ids }),
        Edits::Depth { path, levels, map: None } => (path?, levels.action()),
        Edits::Parallax { path, look } => (path, look.action()),
        Edits::Fit { path, mode } => (path, Action::Fit { mode }),
        Edits::Show { .. }
        | Edits::Apply { .. }
        | Edits::Preview { .. }
        | Edits::Export { .. }
        | Edits::Undo { .. }
        | Edits::Redo { .. }
        | Edits::Depth { map: Some(_), .. } => {
            return None;
        }
    })
}

pub fn run(command: Edits) -> Result<()> {
    match command {
        Edits::Show { path } => print!("{}", open(&path)?.to_json()),
        Edits::Reset { path, ids } if ids.is_empty() => match open(&path) {
            Ok(before) => keep(&path, &before, &Edit { source: before.source.clone(), ..Edit::default() })?,
            Err(_) => drop(edit::remove(&path, &edit::dir()).map_err(told)?),
        },
        Edits::Apply { path, file } => apply(&path, &file)?,
        Edits::Preview { path, changes, stop } => preview(&path, changes, stop)?,
        Edits::Undo { path } => step(&path, true)?,
        Edits::Redo { path } => step(&path, false)?,
        Edits::Export { path, out, before, quality, size } => {
            export(&path, &out, before, quality, size.as_deref())?
        }
        Edits::Depth { map: Some(map), .. } => crate::depth::run(map)?,
        command => action(command).map_or(Ok(()), |(path, action)| change(&path, action))?,
    }
    Ok(())
}
